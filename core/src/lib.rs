//! Core logic for the PS5 Drive Enabler.
//!
//! Writes the embedded 2 MiB "Gen3 enabler" image to the start of an NVMe drive
//! so the PS5 skips its PCIe Gen4 speed requirement. Front-ends (CLI and GUI)
//! build on the small API re-exported below.

pub mod disk;
pub mod error;
pub mod flash;
pub mod image;
mod platform;

pub use disk::{human_size, Disk, Risk};
pub use error::{Error, Result};
pub use flash::{flash, flash_and_verify, verify, ProgressEvent};

/// List every whole physical disk visible to the OS.
pub fn list_disks() -> Result<Vec<Disk>> {
    platform::list_disks()
}

/// Find a disk by its short id (`disk4`, `nvme0n1`, `2`) or raw path.
pub fn find_disk(id_or_path: &str) -> Result<Disk> {
    let needle = id_or_path.trim();
    list_disks()?
        .into_iter()
        .find(|d| {
            d.id == needle || d.raw_path == needle || format!("/dev/{}", d.id) == needle
        })
        .ok_or_else(|| Error::DeviceNotFound(needle.to_string()))
}

/// The embedded enabler image written to drives by default.
pub fn embedded_image() -> &'static [u8] {
    image::embedded()
}
