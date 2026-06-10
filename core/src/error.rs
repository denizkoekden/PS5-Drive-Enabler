use std::fmt;

/// Errors produced by the core library.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O failure with extra context.
    Io { context: String, source: std::io::Error },
    /// The OS refused access, almost always "not running as root/Administrator".
    Permission(String),
    /// Disk enumeration failed (e.g. a helper command could not be run).
    Enumerate(String),
    /// The requested device could not be found among the connected disks.
    DeviceNotFound(String),
    /// The device is in use / locked and could not be opened or unmounted.
    Busy(String),
    /// The supplied image has an invalid size or shape.
    InvalidImage(String),
    /// Read-back verification mismatch at a given byte offset.
    VerifyMismatch { offset: u64 },
    /// Generic catch-all.
    Other(String),
}

impl Error {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        // Map permission-denied at the boundary so callers get a clear message.
        if source.kind() == std::io::ErrorKind::PermissionDenied {
            Error::Permission(context.into())
        } else {
            Error::Io { context: context.into(), source }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { context, source } => write!(f, "{context}: {source}"),
            Error::Permission(ctx) => write!(
                f,
                "permission denied ({ctx}). Re-run this tool with administrator rights \
                 (Windows: \"Run as administrator\"; macOS/Linux: prefix the command with sudo)."
            ),
            Error::Enumerate(m) => write!(f, "could not list disks: {m}"),
            Error::DeviceNotFound(m) => write!(f, "device not found: {m}"),
            Error::Busy(m) => write!(f, "device is busy: {m}"),
            Error::InvalidImage(m) => write!(f, "invalid image: {m}"),
            Error::VerifyMismatch { offset } => {
                write!(f, "verification failed: data differs at byte offset {offset}")
            }
            Error::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
