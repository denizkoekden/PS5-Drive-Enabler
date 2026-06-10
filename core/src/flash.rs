use crate::disk::Disk;
use crate::error::{Error, Result};
use crate::image;
use crate::platform;
use std::io::{Read, Seek, SeekFrom, Write};

/// Events emitted during a flash or verify so front-ends can show progress.
pub enum ProgressEvent<'a> {
    /// A new phase started (human-readable, e.g. "Writing enabler image").
    Stage(&'a str),
    /// Byte-level progress within the current phase.
    Progress { done: u64, total: u64 },
}

/// 1 MiB transfer chunk, a multiple of every common sector size, so raw device
/// I/O stays block-aligned on all platforms.
const CHUNK: usize = 1024 * 1024;

/// Write `image` to the very start (LBA 0) of `disk`, then flush to hardware.
///
/// This unmounts/offlines the device first (platform-specific) and requires
/// administrator/root privileges. It overwrites the first bytes of the disk
/// unconditionally; the caller is responsible for confirming the target.
pub fn flash(disk: &Disk, image: &[u8], progress: &mut dyn FnMut(ProgressEvent)) -> Result<()> {
    image::validate(image)?;

    progress(ProgressEvent::Stage("Preparing device"));
    let mut dev = platform::open_for_write(disk)?;

    write_image(&mut dev, image, progress)?;

    progress(ProgressEvent::Stage("Flushing to hardware"));
    if let Err(e) = dev.sync_all() {
        // Raw character devices (notably macOS /dev/rdiskN) are unbuffered and
        // reject fsync with ENOTTY/EINVAL; the data is already committed by
        // write(), so those codes are benign. Any other error is a real flush
        // failure (e.g. on Linux block devices or Windows physical drives).
        if !matches!(e.raw_os_error(), Some(25) | Some(22)) {
            return Err(Error::io("syncing device", e));
        }
    }
    Ok(())
}

/// Read the first `image.len()` bytes back from `disk` and confirm they match
/// `image` byte-for-byte.
pub fn verify(disk: &Disk, image: &[u8], progress: &mut dyn FnMut(ProgressEvent)) -> Result<()> {
    image::validate(image)?;
    let mut dev = platform::open_for_read(disk)?;
    read_verify(&mut dev, image, progress)
}

/// Convenience: flash then verify in one call.
pub fn flash_and_verify(
    disk: &Disk,
    image: &[u8],
    progress: &mut dyn FnMut(ProgressEvent),
) -> Result<()> {
    flash(disk, image, progress)?;
    verify(disk, image, progress)
}

/// Seek to the start of `dev` and write `image`, reporting progress. Kept
/// generic over the writer so it can be tested without a real device.
fn write_image<W: Write + Seek>(
    dev: &mut W,
    image: &[u8],
    progress: &mut dyn FnMut(ProgressEvent),
) -> Result<()> {
    progress(ProgressEvent::Stage("Writing enabler image"));
    dev.seek(SeekFrom::Start(0))
        .map_err(|e| Error::io("seeking to start of device", e))?;

    let total = image.len() as u64;
    let mut written = 0u64;
    for chunk in image.chunks(CHUNK) {
        dev.write_all(chunk)
            .map_err(|e| Error::io("writing to device", e))?;
        written += chunk.len() as u64;
        progress(ProgressEvent::Progress { done: written, total });
    }
    dev.flush().map_err(|e| Error::io("flushing device", e))?;
    Ok(())
}

/// Seek to the start of `dev` and confirm its first bytes equal `image`.
fn read_verify<R: Read + Seek>(
    dev: &mut R,
    image: &[u8],
    progress: &mut dyn FnMut(ProgressEvent),
) -> Result<()> {
    progress(ProgressEvent::Stage("Verifying written data"));
    dev.seek(SeekFrom::Start(0))
        .map_err(|e| Error::io("seeking to start of device", e))?;

    let total = image.len() as u64;
    let mut buf = vec![0u8; CHUNK];
    let mut offset = 0u64;
    for chunk in image.chunks(CHUNK) {
        let n = chunk.len();
        dev.read_exact(&mut buf[..n])
            .map_err(|e| Error::io("reading back from device", e))?;
        if buf[..n] != *chunk {
            let mut i = 0;
            while i < n && buf[i] == chunk[i] {
                i += 1;
            }
            return Err(Error::VerifyMismatch { offset: offset + i as u64 });
        }
        offset += n as u64;
        progress(ProgressEvent::Progress { done: offset, total });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn noop(_: ProgressEvent) {}

    #[test]
    fn round_trip_matches() {
        // 2 MiB of varied data, like the real enabler image.
        let image: Vec<u8> = (0..2 * 1024 * 1024).map(|i| (i * 31 + 7) as u8).collect();
        let mut backing = Cursor::new(vec![0u8; image.len()]);

        write_image(&mut backing, &image, &mut noop).unwrap();
        read_verify(&mut backing, &image, &mut noop).unwrap();
    }

    #[test]
    fn verify_detects_corruption() {
        let image: Vec<u8> = (0..2 * 1024 * 1024).map(|i| i as u8).collect();
        let mut backing = Cursor::new(image.clone());
        // Flip a byte in the second chunk.
        backing.get_mut()[1_500_000] ^= 0xFF;

        match read_verify(&mut backing, &image, &mut noop) {
            Err(Error::VerifyMismatch { offset }) => assert_eq!(offset, 1_500_000),
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[test]
    fn progress_reaches_total() {
        let image = vec![0xABu8; 2 * 1024 * 1024];
        let mut backing = Cursor::new(vec![0u8; image.len()]);
        let mut last = 0u64;
        let total = image.len() as u64;
        write_image(&mut backing, &image, &mut |ev| {
            if let ProgressEvent::Progress { done, total: t } = ev {
                assert_eq!(t, total);
                last = done;
            }
        })
        .unwrap();
        assert_eq!(last, total);
    }
}
