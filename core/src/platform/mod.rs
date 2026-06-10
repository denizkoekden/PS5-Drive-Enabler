//! Platform-specific disk enumeration and raw device access.
//!
//! Each backend exposes the same three functions; the rest of the crate is
//! platform-agnostic.

use crate::disk::Disk;
use crate::error::Result;
use std::fs::File;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// List every whole physical disk the OS can see.
pub fn list_disks() -> Result<Vec<Disk>> {
    #[cfg(target_os = "linux")]
    {
        linux::list_disks()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_disks()
    }
    #[cfg(target_os = "windows")]
    {
        windows::list_disks()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(crate::error::Error::Other(
            "this operating system is not supported".into(),
        ))
    }
}

/// Open a disk for raw read+write, performing any unmount/offline needed first.
pub fn open_for_write(disk: &Disk) -> Result<File> {
    #[cfg(target_os = "linux")]
    {
        linux::open_for_write(disk)
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_for_write(disk)
    }
    #[cfg(target_os = "windows")]
    {
        windows::open_for_write(disk)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = disk;
        Err(crate::error::Error::Other(
            "this operating system is not supported".into(),
        ))
    }
}

/// Open a disk for raw read-only access (used by verify).
pub fn open_for_read(disk: &Disk) -> Result<File> {
    #[cfg(target_os = "linux")]
    {
        linux::open_for_read(disk)
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_for_read(disk)
    }
    #[cfg(target_os = "windows")]
    {
        windows::open_for_read(disk)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = disk;
        Err(crate::error::Error::Other(
            "this operating system is not supported".into(),
        ))
    }
}
