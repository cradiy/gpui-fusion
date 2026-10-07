use crate::{
    bridge::Host,
    file::{Document, selected_file},
};
use anyhow::Result;
use futures::future::LocalBoxFuture;
use gpui::gpui_io::{
    CreateOptions, FileHandle, FileSystem, IoExecutor, LocationHandle, PlatformLocation,
    PlatformLocations, SystemLocation,
};
use jni::objects::{JString, JValue};
use std::{io, path::PathBuf, sync::Arc};

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
