//! File handles, asynchronous I/O sessions, and platform storage locations.

mod executor;
mod file;
mod location;
mod native;
mod native_trash;
mod session;
mod transfer;

pub use executor::IoExecutor;
pub use file::*;
pub use location::*;
pub use session::*;
pub use transfer::*;

pub(crate) fn unsupported(message: &str) -> anyhow::Error {
    std::io::Error::new(std::io::ErrorKind::Unsupported, message).into()
}
