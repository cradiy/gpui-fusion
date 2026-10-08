use crate::bridge::Host;
use anyhow::{Result, anyhow, ensure};
use futures::future::LocalBoxFuture;
use gpui::gpui_io::{
    BlockingWrite, FileBookmark, FileHandle, FileMetadata, FileReader, FileWriter, IoExecutor,
    PlatformFile, WriteMode, WriteOptions,
};
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JString, JValue},
};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    os::fd::FromRawFd,
    sync::Arc,
};

pub(crate) struct Document {
    pub vm: Arc<JavaVM>,
    pub object: GlobalRef,
}
impl Document {
    pub fn call<T>(&self, f: impl FnOnce(&mut JNIEnv) -> Result<T>) -> Result<T> {
        let mut env = self.vm.attach_current_thread()?;
        env.with_local_frame(16, |env| {
            let result = f(env);
            if env.exception_check()? {
                let exception = env.exception_occurred()?;
                env.exception_clear()?;
                let description = env
                    .call_method(exception, "toString", "()Ljava/lang/String;", &[])
                    .and_then(|v| v.l())
                    .and_then(|v| env.get_string(&JString::from(v)).map(String::from));
                if env.exception_check()? {
                    env.exception_clear()?;
                }
                return Err(anyhow!(
                    "Android document access failed: {}",
                    description.unwrap_or_else(|_| "provider error".into())
                ));
            }
            result
        })
    }
    pub fn void(&self, method: &str) -> Result<()> {
        self.call(|env| {
            env.call_method(self.object.as_obj(), method, "()V", &[])?;
            Ok(())
        })
    }
}

pub(crate) fn view_intent(
    host: &Host,
    file: &FileHandle,
) -> LocalBoxFuture<'static, Result<GlobalRef>> {
    if let Some(path) = file.path() {
        return crate::file_system::view_path_intent(host, path);
    }
    let Some(file) = file.downcast_ref::<AndroidFile>() else {
        return Box::pin(async {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Android system opening requires a document or published collection file",
            )
            .into())
        });
    };
    let document = file.document.clone();
    file.executor.run(move || {
        document.call(|env| {
            let intent = env
                .call_method(
                    document.object.as_obj(),
                    "viewIntent",
                    "()Landroid/content/Intent;",
                    &[],
                )?
                .l()?;
            Ok(env.new_global_ref(intent)?)
        })
    })
}

pub(crate) fn selected_file(
    vm: Arc<JavaVM>,
    object: GlobalRef,
    executor: IoExecutor,
) -> Result<FileHandle> {
    let document = Arc::new(DocumentLease {
        resource: Arc::new(Document { vm, object }),
        executor: executor.clone(),
    });
    let name = document.call(|env| {
        let value = env
            .call_method(
                document.object.as_obj(),
                "displayName",
                "()Ljava/lang/String;",
                &[],
            )?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })?;
    let writable = document.call(|env| {
        Ok(env
            .call_method(document.object.as_obj(), "canWrite", "()Z", &[])?
            .z()?)
    })?;
    let url = document.call(|env| {
        let value = env
            .call_method(document.object.as_obj(), "url", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })?;
    Ok(FileHandle::new(Arc::new(AndroidFile {
        name,
        url,
        document,
        executor,
        writable,
    })))
}

struct DocumentLease {
    resource: Arc<Document>,
    executor: IoExecutor,
}
impl std::ops::Deref for DocumentLease {
    type Target = Document;
    fn deref(&self) -> &Document {
        &self.resource
    }
}
impl Drop for DocumentLease {
    fn drop(&mut self) {
        let document = self.resource.clone();
        self.executor.dispatch(move || {
            if let Err(error) = document.void("discardPending") {
                log::warn!("Pending file cleanup failed: {error:#}");
            }
        });
    }
}

struct AndroidFile {
    name: String,
    url: String,
    document: Arc<DocumentLease>,
    executor: IoExecutor,
    writable: bool,
}
impl std::fmt::Debug for AndroidFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidFile")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
impl PlatformFile for AndroidFile {
    fn can_trash(&self) -> LocalBoxFuture<'static, Result<bool>> {
        let document = self.document.clone();
        self.executor.run(move || {
            document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "canTrash", "()Z", &[])?
                    .z()?)
            })
        })
    }
    fn trash(&self) -> LocalBoxFuture<'static, Result<()>> {
        let document = self.document.clone();
        self.executor.run(move || {
            let supported = document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "canTrash", "()Z", &[])?
                    .z()?)
            })?;
            if !supported {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "file provider does not support trash",
                )
                .into());
            }
            document.void("trash")
        })
    }
    fn can_rename(&self) -> LocalBoxFuture<'static, Result<bool>> {
        let document = self.document.clone();
        self.executor.run(move || {
            document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "canRename", "()Z", &[])?
                    .z()?)
            })
        })
    }
    fn rename(&self, new_name: String) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let document = self.document.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let object = document.call(|env| {
                let name = env.new_string(new_name)?;
                let object = env
                    .call_method(
                        document.object.as_obj(),
                        "rename",
                        "(Ljava/lang/String;)Ldev/gpui/android/SelectedDocument;",
                        &[JValue::Object(name.as_ref())],
                    )?
                    .l()?;
                Ok(env.new_global_ref(object)?)
            })?;
            selected_file(document.vm.clone(), object, executor)
        })
    }
    fn can_delete(&self) -> LocalBoxFuture<'static, Result<bool>> {
        let document = self.document.clone();
        self.executor.run(move || {
            document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "canDelete", "()Z", &[])?
                    .z()?)
            })
        })
    }
    fn delete(&self) -> LocalBoxFuture<'static, Result<()>> {
        let document = self.document.clone();
        self.executor.run(move || document.void("delete"))
    }
    fn persist(&self) -> LocalBoxFuture<'static, Result<FileBookmark>> {
        let document = self.document.clone();
        let writable = self.writable;
        self.executor.run(move || {
            let uri: String = document.call(|env| {
                let value = env
                    .call_method(
                        document.object.as_obj(),
                        "persist",
                        "()Ljava/lang/String;",
                        &[],
                    )?
                    .l()?;
                Ok(env.get_string(&JString::from(value))?.into())
            })?;
            let mut data = vec![u8::from(writable)];
            data.extend_from_slice(uri.as_bytes());
            Ok(FileBookmark::new("android-document", data))
        })
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn can_write(&self) -> bool {
        self.writable
    }
    fn metadata(&self) -> LocalBoxFuture<'static, Result<FileMetadata>> {
        let document = self.document.clone();
        self.executor.run(move || {
            document.call(|env| {
                let length = env
                    .call_method(document.object.as_obj(), "byteLength", "()J", &[])?
                    .j()?;
                let mime = env
                    .call_method(
                        document.object.as_obj(),
                        "mimeType",
                        "()Ljava/lang/String;",
                        &[],
                    )?
                    .l()?;
                let mime_type = if mime.is_null() {
                    None
                } else {
                    Some(env.get_string(&JString::from(mime))?.into())
                };
                Ok(FileMetadata {
                    byte_len: u64::try_from(length).ok(),
                    mime_type,
                    modified: None,
                })
            })
        })
    }
    fn open_read(&self) -> LocalBoxFuture<'static, Result<FileReader>> {
        let document = self.document.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let fd = document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "openRead", "()I", &[])?
                    .i()?)
            })?;
            ensure!(fd >= 0, "invalid document descriptor");
            // SAFETY: openRead transfers ownership via ParcelFileDescriptor.detachFd.
            let file = unsafe { File::from_raw_fd(fd) };
            Ok(FileReader::from_seekable(
                DocumentReader {
                    file,
                    _lease: document,
                },
                executor,
            ))
        })
    }
    fn open_write(&self, options: WriteOptions) -> LocalBoxFuture<'static, Result<FileWriter>> {
        if !self.writable {
            return Box::pin(async {
                Err(
                    io::Error::new(io::ErrorKind::PermissionDenied, "file handle is read-only")
                        .into(),
                )
            });
        }
        if options.mode != WriteMode::Truncate {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "document append is not supported",
                )
                .into())
            });
        }
        let document = self.document.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let object = document.call(|env| {
                let stream = env
                    .call_method(
                        document.object.as_obj(),
                        "openWrite",
                        "()Ldev/gpui/android/DocumentOutput;",
                        &[],
                    )?
                    .l()?;
                Ok(env.new_global_ref(stream)?)
            })?;
            Ok(FileWriter::from_blocking(
                DocumentOutput {
                    stream: Document {
                        vm: document.vm.clone(),
                        object,
                    },
                    _lease: document,
                },
                executor,
            ))
        })
    }
}

struct DocumentReader {
    file: File,
    _lease: Arc<DocumentLease>,
}
impl Read for DocumentReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Seek for DocumentReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

struct DocumentOutput {
    stream: Document,
    _lease: Arc<DocumentLease>,
}
impl Write for DocumentOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream
            .call(|env| {
                let bytes = env.byte_array_from_slice(bytes)?;
                env.call_method(
                    self.stream.object.as_obj(),
                    "write",
                    "([B)V",
                    &[JValue::Object(bytes.as_ref())],
                )?;
                Ok(())
            })
            .map_err(io::Error::other)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.void("flush").map_err(io::Error::other)
    }
}
impl BlockingWrite for DocumentOutput {
    fn close(&mut self) -> Result<()> {
        self.stream.void("finish")
    }
    fn abort(&mut self) -> Result<()> {
        self.stream.void("abort")
    }
}
