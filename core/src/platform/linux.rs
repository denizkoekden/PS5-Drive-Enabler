use crate::disk::Disk;
use crate::error::{Error, Result};
use std::fs::{File, OpenOptions};
use std::path::Path;

/// Enumerate whole disks by reading `/sys/block`, which needs no privileges and
/// no external tools.
pub fn list_disks() -> Result<Vec<Disk>> {
    let mut disks = Vec::new();
    let entries = std::fs::read_dir("/sys/block")
        .map_err(|e| Error::Enumerate(format!("reading /sys/block: {e}")))?;

    let mounts = read_mounts();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip virtual / non-disk block devices.
        if ["loop", "ram", "zram", "sr", "dm-", "md", "fd", "nbd"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            continue;
        }
        let base = entry.path();
        // `size` is always in 512-byte sectors, independent of physical block size.
        let size = read_u64(&base.join("size")).unwrap_or(0) * 512;
        if size == 0 {
            continue; // empty card reader slot, etc.
        }
        // Many USB-to-SATA/NVMe bridges report removable=0, so also check
        // whether the device sits on a USB controller in the sysfs tree.
        let on_usb = std::fs::canonicalize(&base)
            .map(|p| p.to_string_lossy().contains("/usb"))
            .unwrap_or(false);
        let removable = read_u64(&base.join("removable")).unwrap_or(0) == 1 || on_usb;
        let rotational = read_u64(&base.join("queue/rotational")).unwrap_or(1) == 1;
        let model = read_str(&base.join("device/model"))
            .or_else(|| read_str(&base.join("device/name")))
            .unwrap_or_default();

        let bus = if on_usb {
            "USB".to_string()
        } else if name.starts_with("nvme") {
            "NVMe".to_string()
        } else if removable {
            "USB".to_string()
        } else if rotational {
            "SATA (HDD)".to_string()
        } else {
            "SATA".to_string()
        };

        let system = disk_holds_system(&name, &mounts);

        disks.push(Disk {
            id: name.clone(),
            raw_path: format!("/dev/{name}"),
            model,
            size,
            bus,
            removable,
            internal: !removable,
            system,
        });
    }

    disks.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(disks)
}

pub fn open_for_write(disk: &Disk) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&disk.raw_path)
        .map_err(|e| map_open_err(&disk.raw_path, e))
}

pub fn open_for_read(disk: &Disk) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .open(&disk.raw_path)
        .map_err(|e| map_open_err(&disk.raw_path, e))
}

fn map_open_err(path: &str, e: std::io::Error) -> Error {
    match e.raw_os_error() {
        Some(16) => Error::Busy(format!(
            "{path} is in use. Unmount every partition on it and close any program using it, then try again."
        )),
        _ => Error::io(format!("opening {path}"), e),
    }
}

/// (source_device, mountpoint) pairs from /proc/mounts.
fn read_mounts() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/proc/mounts") {
        for line in text.lines() {
            let mut f = line.split_whitespace();
            if let (Some(src), Some(mp)) = (f.next(), f.next()) {
                out.push((src.to_string(), mp.to_string()));
            }
        }
    }
    out
}

/// True if any partition of `disk_name` is mounted at a core system path.
fn disk_holds_system(disk_name: &str, mounts: &[(String, String)]) -> bool {
    for (src, mp) in mounts {
        let dev = match src.strip_prefix("/dev/") {
            Some(d) => d,
            None => continue,
        };
        if partition_belongs(dev, disk_name)
            && (mp == "/" || mp.starts_with("/boot") || mp == "/usr" || mp == "/var")
        {
            return true;
        }
    }
    false
}

/// Does block device `dev` (e.g. "nvme0n1p2", "sda1") belong to whole disk `disk`?
fn partition_belongs(dev: &str, disk: &str) -> bool {
    if dev == disk {
        return true;
    }
    if let Some(rest) = dev.strip_prefix(disk) {
        // "nvme0n1" + "p2"  -> rest = "p2"; "sda" + "1" -> rest = "1"
        return rest.starts_with('p') || rest.chars().next().is_some_and(|c| c.is_ascii_digit());
    }
    false
}

fn read_str(p: &Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_u64(p: &Path) -> Option<u64> {
    read_str(p).and_then(|s| s.parse().ok())
}
