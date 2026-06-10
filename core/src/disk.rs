/// A whole physical disk that the user could write to.
#[derive(Debug, Clone)]
pub struct Disk {
    /// Short, stable identifier shown to the user and accepted on the CLI.
    /// Linux: `nvme0n1` / `sda`. macOS: `disk4`. Windows: `2` (disk number).
    pub id: String,
    /// The path passed to the OS to open the raw device for I/O.
    /// Linux: `/dev/nvme0n1`. macOS: `/dev/rdisk4`. Windows: `\\.\PhysicalDrive2`.
    pub raw_path: String,
    /// Human-friendly model/product string, if the OS exposed one.
    pub model: String,
    /// Capacity in bytes (0 if unknown).
    pub size: u64,
    /// Connection bus as reported by the OS (e.g. "USB", "NVMe", "SATA").
    pub bus: String,
    /// Whether the OS considers the media removable/external.
    pub removable: bool,
    /// Whether the OS considers the disk an internal/built-in device.
    pub internal: bool,
    /// True if this disk currently holds the running operating system or a
    /// mounted system volume. Writing here would brick the user's computer, so
    /// the tool refuses unless explicitly forced.
    pub system: bool,
}

impl Disk {
    /// Capacity rendered as a friendly decimal-GB string (matches how drives
    /// are advertised), e.g. "1.02 TB" or "931.51 GB".
    pub fn size_pretty(&self) -> String {
        human_size(self.size)
    }

    /// A one-line risk classification used by both front-ends.
    pub fn risk(&self) -> Risk {
        if self.system {
            Risk::System
        } else if self.internal && !self.removable {
            Risk::Internal
        } else {
            Risk::Removable
        }
    }
}

/// How dangerous it is to write to a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Holds the OS and must never be touched.
    System,
    /// Built-in disk, probably not the PS5 drive; require an explicit override.
    Internal,
    /// External/removable, the expected place for a PS5 drive in a USB case.
    Removable,
}

/// Format a byte count the way storage vendors advertise capacity (powers of
/// 1000), which is what users see printed on the drive.
pub fn human_size(bytes: u64) -> String {
    if bytes == 0 {
        return "unknown size".to_string();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}
