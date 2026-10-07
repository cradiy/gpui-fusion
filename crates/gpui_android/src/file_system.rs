use crate::{
    bridge::Host,
    file::{Document, selected_file},
};
use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use gpui::gpui_io::{
    CreateOptions, FileBookmark, FileHandle, FileSystem, IoExecutor, LocationHandle,
    PlatformLocation, PlatformLocations, SystemLocation,
};
use jni::objects::{GlobalRef, JString, JValue};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) fn view_path_intent(
    host: &Host,
    path: &Path,
) -> LocalBoxFuture<'static, Result<GlobalRef>> {
    let store = host.file_store();
    let path = path.to_path_buf();
    crate::dispatcher::io_executor().run(move || {
        let (vm, object) = store?;
        let store = Document { vm, object };
        let path = path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Android file paths must be valid UTF-8",
            )
        })?;
        store.call(|env| {
            let path = env.new_string(path)?;
            let intent = env
                .call_method(
                    store.object.as_obj(),
                    "viewPathIntent",
                    "(Ljava/lang/String;)Landroid/content/Intent;",
                    &[JValue::Object(path.as_ref())],
                )?
                .l()?;
            Ok(env.new_global_ref(intent)?)
        })
    })
}

pub(crate) fn file_system(host: &Host, executor: IoExecutor) -> Result<FileSystem> {
    let (vm, object) = host.file_store()?;
    Ok(FileSystem::new(AndroidLocations {
        store: Arc::new(Document { vm, object }),
        executor,
    }))
}

pub(crate) fn no_backup(
    host: &Host,
    executor: IoExecutor,
) -> Result<LocalBoxFuture<'static, Result<LocationHandle>>> {
    let (vm, object) = host.file_store()?;
    Ok(private_location(
        Arc::new(Document { vm, object }),
        executor,
        3,
    ))
}

fn private_location(
    store: Arc<Document>,
    executor: IoExecutor,
    kind: i32,
) -> LocalBoxFuture<'static, Result<LocationHandle>> {
    let worker = executor.clone();
    executor.run(move || {
        let path: String = store.call(|env| {
            let value = env
                .call_method(
                    store.object.as_obj(),
                    "privatePath",
                    "(I)Ljava/lang/String;",
                    &[JValue::Int(kind)],
                )?
                .l()?;
            Ok(env.get_string(&JString::from(value))?.into())
        })?;
        Ok(LocationHandle::from_path(PathBuf::from(path), worker))
    })
}

struct AndroidLocations {
    store: Arc<Document>,
    executor: IoExecutor,
}
impl PlatformLocations for AndroidLocations {
    fn restore_file(&self, bookmark: FileBookmark) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let store = self.store.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let (uri, writable) = document_bookmark(&bookmark)?;
            let object = store.call(|env| {
                let uri = env.new_string(uri)?;
                let file = env
                    .call_method(
                        store.object.as_obj(),
                        "restore",
                        "(Ljava/lang/String;Z)Ldev/gpui/android/SelectedDocument;",
                        &[JValue::Object(uri.as_ref()), JValue::Bool(writable.into())],
                    )?
                    .l()?;
                Ok(env.new_global_ref(file)?)
            })?;
            selected_file(store.vm.clone(), object, executor)
        })
    }
    fn release_file(&self, bookmark: FileBookmark) -> LocalBoxFuture<'static, Result<()>> {
        let store = self.store.clone();
        self.executor.run(move || {
            let (uri, writable) = document_bookmark(&bookmark)?;
            store.call(|env| {
                let uri = env.new_string(uri)?;
                env.call_method(
                    store.object.as_obj(),
                    "release",
                    "(Ljava/lang/String;Z)V",
                    &[JValue::Object(uri.as_ref()), JValue::Bool(writable.into())],
                )?;
                Ok(())
            })
        })
    }
    fn location(&self, kind: SystemLocation) -> LocalBoxFuture<'static, Result<LocationHandle>> {
        let code = match kind {
            SystemLocation::AppData => {
                return private_location(self.store.clone(), self.executor.clone(), 0);
            }
            SystemLocation::AppConfig => {
                return private_location(self.store.clone(), self.executor.clone(), 1);
            }
            SystemLocation::Cache => {
                return private_location(self.store.clone(), self.executor.clone(), 2);
            }
            SystemLocation::Downloads => 0,
            SystemLocation::Pictures => 1,
            SystemLocation::Music => 2,
            SystemLocation::Videos => 3,
            SystemLocation::Documents => {
                return Box::pin(async {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Documents requires user selection through the document picker",
                    )
                    .into())
                });
            }
        };
        let store = self.store.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let supported = store.call(|env| {
                Ok(env
                    .call_method(store.object.as_obj(), "collectionsSupported", "()Z", &[])?
                    .z()?)
            })?;
            if !supported {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "public collections require Android 10 or later",
                )
                .into());
            }
            Ok(LocationHandle::new(Collection {
                store,
                executor,
                code,
            }))
        })
    }
}

fn document_bookmark(bookmark: &FileBookmark) -> Result<(&str, bool)> {
    ensure!(
        bookmark.provider() == "android-document",
        "unsupported file bookmark provider"
    );
    let data = bookmark.data();
    ensure!(
        (2..=16385).contains(&data.len()) && data[0] <= 1,
        "invalid document bookmark"
    );
    Ok((std::str::from_utf8(&data[1..])?, data[0] == 1))
}

struct Collection {
    store: Arc<Document>,
    executor: IoExecutor,
    code: i32,
}
impl PlatformLocation for Collection {
    fn create_file(
        &self,
        name: String,
        options: CreateOptions,
    ) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let store = self.store.clone();
        let executor = self.executor.clone();
        let code = self.code;
        self.executor.run(move || {
            let mime = options.mime_type.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a concrete MIME type is required for public collections"))?;
            let object = store.call(|env| {
                let name = env.new_string(name)?;
                let mime = env.new_string(mime)?;
                let file = env.call_method(store.object.as_obj(), "create", "(ILjava/lang/String;Ljava/lang/String;)Ldev/gpui/android/SelectedDocument;",
                    &[JValue::Int(code), JValue::Object(name.as_ref()), JValue::Object(mime.as_ref())])?.l()?;
                Ok(env.new_global_ref(file)?)
            })?;
            selected_file(store.vm.clone(), object, executor)
        })
    }
}
