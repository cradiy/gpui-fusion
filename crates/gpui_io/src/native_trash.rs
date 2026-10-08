use anyhow::Result;
use std::path::Path;

#[cfg(target_os = "linux")]
pub(crate) fn trash(path: &Path) -> Result<()> {
    Ok(trash::delete(path)?)
}

#[cfg(target_os = "macos")]
pub(crate) fn trash(path: &Path) -> Result<()> {
    use objc2_foundation::{NSFileManager, NSURL};
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(std::path::absolute(path)?.as_os_str().as_bytes())?;
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: path is a live, NUL-terminated filesystem representation.
        let url = unsafe {
            NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
                std::ptr::NonNull::new(path.as_ptr().cast_mut())
                    .expect("CString pointer is non-null"),
                false,
                None,
            )
        };
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|error| anyhow::anyhow!("Unable to move file to Trash: {error}"))
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn trash(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{System::Com::*, UI::Shell::*},
        core::PCWSTR,
    };

    // Shell operations require their own STA, independent of the injected worker's COM mode.
    let path = std::fs::canonicalize(path)?;
    std::thread::Builder::new()
        .name("gpui-trash".into())
        .spawn(move || -> Result<()> {
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) {
                    // SAFETY: balanced with successful initialization on this thread.
                    unsafe { CoUninitialize() };
                }
            }
            // SAFETY: this newly created thread has no existing COM apartment.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
            let _apartment = Apartment;
            let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
            let prefix: Vec<u16> = "\\\\?\\UNC\\".encode_utf16().collect();
            if path.starts_with(&prefix) {
                path.splice(..prefix.len(), "\\\\".encode_utf16());
            } else if path.starts_with(&"\\\\?\\".encode_utf16().collect::<Vec<_>>()) {
                path.drain(..4);
            }
            path.push(0);
            // SAFETY: COM is initialized; the path buffer and interfaces outlive all calls.
            unsafe {
                let operation: IFileOperation =
                    CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
                operation.SetOperationFlags(
                    FOF_SILENT
                        | FOF_NOCONFIRMATION
                        | FOF_NOERRORUI
                        | FOF_NO_CONNECTED_ELEMENTS
                        | FOF_WANTNUKEWARNING
                        | FOFX_EARLYFAILURE
                        | FOFX_RECYCLEONDELETE
                        | FOFX_ADDUNDORECORD,
                )?;
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)?;
                operation.DeleteItem(&item, None)?;
                operation.PerformOperations()?;
                anyhow::ensure!(
                    !operation.GetAnyOperationsAborted()?.as_bool(),
                    "Recycle operation was aborted"
                );
            }
            Ok(())
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("Recycle worker panicked"))?
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn trash(_: &Path) -> Result<()> {
    Err(crate::unsupported(
        "native files have no system trash on this platform",
    ))
}
