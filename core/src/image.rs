use crate::error::{Error, Result};
use std::path::Path;

/// The Gen3 enabler payload, embedded into the binary at compile time so the
/// shipped tool is a single self-contained file.
///
/// It is written verbatim to the first bytes (LBA 0) of the target NVMe drive.
/// The PS5 reads this region during the drive-speed check; once present it
/// skips the PCIe Gen4 requirement.
pub const EMBEDDED_IMAGE: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../gen3_enabler_bringus.img"));

/// Expected size of the enabler image: exactly 2 MiB.
pub const EXPECTED_IMAGE_LEN: usize = 2 * 1024 * 1024;

// Reject a malformed image file at compile time rather than at flash time.
const _: () = assert!(EMBEDDED_IMAGE.len() == EXPECTED_IMAGE_LEN);

/// Block size everything is aligned to. Raw device I/O on all three platforms
/// is happy with 4096-byte-aligned reads and writes, and 2 MiB is a multiple
/// of it.
pub const BLOCK_SIZE: u64 = 4096;

/// Validate that an image is usable for raw device writes.
pub fn validate(image: &[u8]) -> Result<()> {
    if image.is_empty() {
        return Err(Error::InvalidImage("image is empty".into()));
    }
    if !(image.len() as u64).is_multiple_of(BLOCK_SIZE) {
        return Err(Error::InvalidImage(format!(
            "image length {} is not a multiple of the {BLOCK_SIZE}-byte block size; \
             raw device writes require block alignment",
            image.len()
        )));
    }
    Ok(())
}

/// Load an override image from disk, validating its shape.
pub fn load_from_path(path: &Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path)
        .map_err(|e| Error::io(format!("reading image file {}", path.display()), e))?;
    validate(&data)?;
    Ok(data)
}

/// Return the embedded image. Its size is checked at compile time.
pub fn embedded() -> &'static [u8] {
    EMBEDDED_IMAGE
}
