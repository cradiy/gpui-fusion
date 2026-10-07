use anyhow::{Result, anyhow};
use futures::future::LocalBoxFuture;
use gpui::gpui_io::{FileHandle, FileMetadata, FileReader, PlatformFile, PlatformReader};
use gpui_util::browser::BrowserResource;
use std::{io::SeekFrom, sync::Arc};

#[derive(Debug)]
struct FileResource {
    file: web_sys::File,
    url: String,
}
impl Drop for FileResource {
    fn drop(&mut self) {
        let _ = web_sys::Url::revoke_object_url(&self.url);
    }
}
#[derive(Debug)]
struct BrowserFile {
    name: String,
    url: String,
    resource: BrowserResource<FileResource>,
}
impl PlatformFile for BrowserFile {
    fn name(&self) -> &str {
        &self.name
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn metadata(&self) -> LocalBoxFuture<'static, Result<FileMetadata>> {
        let resource = self.resource.clone();
        Box::pin(async move {
            resource
                .with(|r| FileMetadata {
                    byte_len: Some(r.file.size() as u64),
                    mime_type: match r.file.type_() {
                        mime if mime.is_empty() => None,
                        mime => Some(mime),
                    },
                    modified: std::time::Duration::try_from_secs_f64(
                        r.file.last_modified() / 1000.,
                    )
                    .ok()
                    .and_then(|d| std::time::UNIX_EPOCH.checked_add(d)),
                })
                .ok_or_else(|| anyhow!("browser file must be accessed on its owning thread"))
        })
    }
    fn open_read(&self) -> LocalBoxFuture<'static, Result<FileReader>> {
        let resource = self.resource.clone();
        Box::pin(async move {
            let size = resource
                .with(|r| r.file.size() as u64)
                .ok_or_else(|| anyhow!("browser file must be opened on its owning thread"))?;
            Ok(FileReader::new(BrowserReader {
                resource,
                offset: 0,
                size,
            }))
        })
    }
}

struct BrowserReader {
    resource: BrowserResource<FileResource>,
    offset: u64,
    size: u64,
}
impl PlatformReader for BrowserReader {
    fn seek(&mut self, position: SeekFrom) -> LocalBoxFuture<'_, Result<u64>> {
        Box::pin(async move {
            self.resource
                .with(|_| ())
                .ok_or_else(|| anyhow!("browser file must be accessed on its owning thread"))?;
            let offset = match position {
                SeekFrom::Start(offset) => i128::from(offset),
                SeekFrom::Current(delta) => i128::from(self.offset) + i128::from(delta),
                SeekFrom::End(delta) => i128::from(self.size) + i128::from(delta),
            };
            let offset = u64::try_from(offset).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "seek position is outside the byte offset range",
                )
            })?;
            self.offset = offset;
            Ok(offset)
        })
    }
    fn read_chunk(&mut self, limit: usize) -> LocalBoxFuture<'_, Result<Option<Vec<u8>>>> {
        Box::pin(async move {
            if self.offset >= self.size {
                return Ok(None);
            }
            let end = self.offset.saturating_add(limit as u64).min(self.size);
            let blob = self
                .resource
                .with(|r| {
                    r.file
                        .slice_with_f64_and_f64(self.offset as f64, end as f64)
                })
                .ok_or_else(|| anyhow!("browser file must be read on its owning thread"))?
                .map_err(|error| anyhow!("browser file read: {error:?}"))?;
            let buffer = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
                .await
                .map_err(|error| anyhow!("browser file read: {error:?}"))?;
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            self.offset = end;
            Ok(Some(bytes))
        })
    }
}

pub(crate) fn selected_file(file: web_sys::File) -> Result<FileHandle> {
    let name = file.name();
    let url = web_sys::Url::create_object_url_with_blob(&file)
        .map_err(|e| anyhow!("browser file URL: {e:?}"))?;
    Ok(FileHandle::new(Arc::new(BrowserFile {
        name,
        url: url.clone(),
        resource: BrowserResource::new(FileResource { file, url }),
    })))
}
