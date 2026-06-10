use crate::disk::Disk;
use crate::error::{Error, Result};
use std::fs::{File, OpenOptions};
use std::process::Command;

/// Enumerate disks through PowerShell's `Get-Disk`. We request a strict
/// pipe-delimited line per disk so no JSON parser is needed.
pub fn list_disks() -> Result<Vec<Disk>> {
    let script = "Get-Disk | ForEach-Object { \
        \"$($_.Number)|$($_.FriendlyName)|$($_.Size)|$($_.BusType)|$($_.IsBoot)|$($_.IsSystem)|$($_.OperationalStatus)\" }";
    let out = run_ps(script)
        .map_err(|e| Error::Enumerate(format!("running PowerShell Get-Disk: {e}")))?;
    if !out.status.success() {
        return Err(Error::Enumerate(format!(
            "Get-Disk failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut disks = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        if f.len() < 7 {
            continue;
        }
        let number: u32 = match f[0].trim().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let model = f[1].trim().to_string();
        let size: u64 = f[2].trim().parse().unwrap_or(0);
        let bus = f[3].trim().to_string();
        let is_boot = parse_bool(f[4]);
        let is_system = parse_bool(f[5]);
        let removable = bus.eq_ignore_ascii_case("USB");

        disks.push(Disk {
            id: number.to_string(),
            raw_path: format!(r"\\.\PhysicalDrive{number}"),
            model,
            size,
            bus,
            removable,
            internal: !removable,
            system: is_boot || is_system,
        });
    }
    Ok(disks)
}

pub fn open_for_write(disk: &Disk) -> Result<File> {
    // Offline + clear read-only so raw writes to the physical device go through.
    // Best-effort: a brand-new PS5 drive is often already offline/uninitialized.
    let n = &disk.id;
    let _ = run_ps(&format!("Set-Disk -Number {n} -IsReadOnly $false"));
    let _ = run_ps(&format!("Set-Disk -Number {n} -IsOffline $true"));

    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&disk.raw_path)
        .map_err(|e| Error::io(format!("opening {}", disk.raw_path), e))
}

pub fn open_for_read(disk: &Disk) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .open(&disk.raw_path)
        .map_err(|e| Error::io(format!("opening {}", disk.raw_path), e))
}

fn run_ps(cmd: &str) -> std::io::Result<std::process::Output> {
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
        .output()
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim(), "True" | "true" | "1")
}
