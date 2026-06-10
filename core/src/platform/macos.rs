use crate::disk::Disk;
use crate::error::{Error, Result};
use plist::Value;
use std::fs::{File, OpenOptions};
use std::process::Command;

/// Enumerate physical disks via `diskutil`, which ships with every macOS.
pub fn list_disks() -> Result<Vec<Disk>> {
    let out = Command::new("diskutil")
        .args(["list", "-plist", "physical"])
        .output()
        .map_err(|e| Error::Enumerate(format!("running diskutil: {e}")))?;

    // Older macOS may not accept the "physical" filter; fall back to a plain list.
    let stdout = if out.status.success() {
        out.stdout
    } else {
        Command::new("diskutil")
            .args(["list", "-plist"])
            .output()
            .map_err(|e| Error::Enumerate(format!("running diskutil: {e}")))?
            .stdout
    };

    let value: Value = plist::from_bytes(&stdout)
        .map_err(|e| Error::Enumerate(format!("parsing diskutil output: {e}")))?;
    let dict = value
        .as_dictionary()
        .ok_or_else(|| Error::Enumerate("unexpected diskutil output".into()))?;

    let mut ids = Vec::new();
    if let Some(arr) = dict.get("AllDisksAndPartitions").and_then(|x| x.as_array()) {
        for d in arr {
            if let Some(id) = d
                .as_dictionary()
                .and_then(|m| m.get("DeviceIdentifier"))
                .and_then(|x| x.as_string())
            {
                ids.push(id.to_string());
            }
        }
    }

    let boot = boot_disk_ids();
    let mut disks = Vec::new();
    for id in ids {
        if let Some(disk) = disk_info(&id, &boot)? {
            disks.push(disk);
        }
    }
    Ok(disks)
}

/// Whole-disk identifiers backing the boot volume ("/").
///
/// On APFS the root volume lives on a synthesized disk (e.g. disk3) whose
/// physical store is a partition like disk0s2, so OSInternalMedia is not set
/// on the physical whole disk itself. Resolve the chain explicitly.
fn boot_disk_ids() -> Vec<String> {
    let out = match Command::new("diskutil").args(["info", "-plist", "/"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let value: Value = match plist::from_bytes(&out.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let d = match value.as_dictionary() {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut ids = Vec::new();
    if let Some(parent) = d.get("ParentWholeDisk").and_then(|x| x.as_string()) {
        ids.push(parent.to_string());
    }
    if let Some(stores) = d.get("APFSPhysicalStores").and_then(|x| x.as_array()) {
        for s in stores {
            if let Some(part) = s
                .as_dictionary()
                .and_then(|m| m.get("APFSPhysicalStore"))
                .and_then(|x| x.as_string())
            {
                ids.push(whole_disk_of(part));
            }
        }
    }
    ids
}

/// Strip the partition suffix from a BSD identifier: "disk0s2" -> "disk0".
fn whole_disk_of(id: &str) -> String {
    match id.strip_prefix("disk") {
        Some(rest) => {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            format!("disk{digits}")
        }
        None => id.to_string(),
    }
}

fn disk_info(id: &str, boot_ids: &[String]) -> Result<Option<Disk>> {
    let out = Command::new("diskutil")
        .args(["info", "-plist", id])
        .output()
        .map_err(|e| Error::Enumerate(format!("running diskutil info {id}: {e}")))?;
    if !out.status.success() {
        return Ok(None);
    }
    let value: Value = match plist::from_bytes(&out.stdout) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let d = match value.as_dictionary() {
        Some(d) => d,
        None => return Ok(None),
    };

    let get_str = |k: &str| d.get(k).and_then(|x| x.as_string()).unwrap_or("").to_string();
    let get_bool = |k: &str| d.get(k).and_then(|x| x.as_boolean()).unwrap_or(false);
    let get_i64 = |k: &str| d.get(k).and_then(|x| x.as_signed_integer()).unwrap_or(0);

    // Only consider whole disks, not partitions or synthesized volumes.
    if !get_bool("WholeDisk") {
        return Ok(None);
    }

    let size = get_i64("Size").max(0) as u64;
    let model = {
        let m = get_str("MediaName");
        if m.trim().is_empty() {
            get_str("IORegistryEntryName")
        } else {
            m
        }
    };
    let internal = get_bool("Internal");
    let removable = get_bool("RemovableMedia") || get_bool("RemovableMediaOrExternalDevice");
    let bus = get_str("BusProtocol");
    // A disk is system-risk when it backs the boot volume. Internal non-OS
    // disks stay writable behind the separate "internal" warning/override.
    let system = get_bool("OSInternalMedia") || boot_ids.iter().any(|b| b == id);

    Ok(Some(Disk {
        id: id.to_string(),
        // /dev/rdiskN is the raw (unbuffered) node: fast and block-aligned.
        raw_path: format!("/dev/r{id}"),
        model: model.trim().to_string(),
        size,
        bus,
        removable,
        internal,
        system,
    }))
}

pub fn open_for_write(disk: &Disk) -> Result<File> {
    // The kernel only lets us write the raw whole-disk node when it is unmounted.
    let _ = Command::new("diskutil")
        .args(["unmountDisk", "force", &format!("/dev/{}", disk.id)])
        .output();
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
            "{path} is busy. Make sure the drive is not mounted, then try again."
        )),
        _ => Error::io(format!("opening {path}"), e),
    }
}
