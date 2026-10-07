//! File handles, asynchronous I/O sessions, and platform storage locations.

mod executor;
mod file;
mod location;
mod native;
mod session;

pub use executor::IoExecutor;
pub use file::*;
pub use location::*;
pub use session::*;

pub(crate) fn unsupported(message: &str) -> anyhow::Error {
    std::io::Error::new(std::io::ErrorKind::Unsupported, message).into()
}
