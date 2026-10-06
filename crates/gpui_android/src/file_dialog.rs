use crate::bridge::Host;
use anyhow::{Result, anyhow, ensure};
use futures::{channel::oneshot, future::LocalBoxFuture};
use gpui::{
    BackgroundExecutor, FilePromptOptions, FileSaveOptions, ForegroundExecutor, PlatformFile,
    SelectedFile,
};
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JString, JValue},
};
use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::Read,
    os::fd::FromRawFd,
    rc::Rc,
    sync::Arc,
};

type Selection = Result<Option<Vec<SelectedFile>>>;
type Pending = (u64, oneshot::Sender<Selection>);

pub(crate) struct FileDialog {
    host: Arc<Host>,
    background: BackgroundExecutor,
    foreground: ForegroundExecutor,
    pending: RefCell<Option<Pending>>,
    metadata: RefCell<Option<gpui::Task<()>>>,
    next: Cell<u64>,
    closed: Cell<bool>,
}

impl FileDialog {
    pub fn new(
        host: Arc<Host>,
        background: BackgroundExecutor,
        foreground: ForegroundExecutor,
    ) -> Rc<Self> {
        Rc::new(Self {
            host,
            background,
            foreground,
            pending: RefCell::default(),
            metadata: RefCell::default(),
            next: Cell::new(0),
            closed: Cell::new(false),
        })
    }

    pub fn prompt(&self, options: FilePromptOptions) -> oneshot::Receiver<Selection> {
        self.prompt_with(|token| {
            self.host
                .request_files(token, options.multiple, options.writable)
        })
    }

    pub fn prompt_save(
        &self,
        options: FileSaveOptions,
    ) -> oneshot::Receiver<Result<Option<SelectedFile>>> {
        let selection = self.prompt_with(|token| self.host.request_file_save(token, &options));
        let (tx, rx) = oneshot::channel();
        self.foreground
            .spawn(async move {
                let result = async {
                    let Some(mut files) = selection.await?? else {
                        return Ok(None);
                    };
                    ensure!(files.len() == 1, "save picker must return one document");
                    Ok(files.pop())
                }
                .await;
                let _ = tx.send(result);
            })
            .detach();
        rx
    }

    fn prompt_with(&self, request: impl FnOnce(u64) -> Result<()>) -> oneshot::Receiver<Selection> {
        let (tx, rx) = oneshot::channel();
        if self.closed.get() || self.pending.borrow().is_some() {
            let _ = tx.send(Err(anyhow!(
                "file selection requires an open session with no pending file dialog"
            )));
            return rx;
        }
        let Some(token) = self.next.get().checked_add(1) else {
            let _ = tx.send(Err(anyhow!("file request identifiers exhausted")));
            return rx;
        };
        self.next.set(token);
        *self.pending.borrow_mut() = Some((token, tx));
        if let Err(error) = request(token) {
            self.finish(token, Err(error));
        }
        rx
    }

    pub fn complete(
        self: &Rc<Self>,
        token: u64,
        vm: Arc<JavaVM>,
        result: Result<Option<Vec<GlobalRef>>>,
    ) {
        if !self
            .pending
            .borrow()
            .as_ref()
            .is_some_and(|(id, _)| *id == token)
        {
            return;
        }
        let objects = match result {
            Ok(Some(objects)) => objects,
            other => {
                self.finish(token, other.map(|_| None));
                return;
            }
        };
        let executor = self.background.clone();
        let task = self.background.spawn(async move {
            objects
                .into_iter()
                .map(|object| {
                    let document = Arc::new(Document {
                        vm: vm.clone(),
                        object,
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
                    Ok(SelectedFile::new(Arc::new(AndroidFile {
                        name,
                        document,
                        executor: executor.clone(),
                        writable,
                        access: Arc::new(parking_lot::Mutex::new(())),
                    })))
                })
                .collect::<Result<Vec<_>>>()
                .map(Some)
        });
        let state = Rc::downgrade(self);
        *self.metadata.borrow_mut() = Some(self.foreground.spawn(async move {
            let result = task.await;
            if let Some(state) = state.upgrade() {
                state.finish(token, result);
            }
        }));
    }

    fn finish(&self, token: u64, result: Selection) {
        let pending = {
            let mut slot = self.pending.borrow_mut();
            if slot.as_ref().is_some_and(|(id, _)| *id == token) {
                slot.take()
            } else {
                None
            }
        };
        if let Some((_, tx)) = pending {
            if let Some(task) = self.metadata.borrow_mut().take() {
                task.detach();
            }
            let _ = tx.send(result);
        }
    }

    pub fn close(&self) {
        self.closed.set(true);
        self.metadata.borrow_mut().take();
        let pending = self.pending.borrow_mut().take();
        if let Some((_, tx)) = pending {
            let _ = tx.send(Err(anyhow!("Android file selection session closed")));
        }
    }
}

struct Document {
    vm: Arc<JavaVM>,
    object: GlobalRef,
}

impl Document {
    fn call<T>(&self, f: impl FnOnce(&mut JNIEnv) -> Result<T>) -> Result<T> {
        let mut env = self.vm.attach_current_thread()?;
        env.with_local_frame(16, |env| {
            let result = f(env);
            if env.exception_check()? {
                let exception = env.exception_occurred()?;
                env.exception_clear()?;
                let description = env
                    .call_method(exception, "toString", "()Ljava/lang/String;", &[])
                    .and_then(|value| value.l())
                    .and_then(|value| env.get_string(&JString::from(value)).map(String::from));
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
}

struct AndroidFile {
    name: String,
    document: Arc<Document>,
    executor: BackgroundExecutor,
    writable: bool,
    access: Arc<parking_lot::Mutex<()>>,
}

impl std::fmt::Debug for AndroidFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidFile")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl PlatformFile for AndroidFile {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> LocalBoxFuture<'static, Result<Vec<u8>>> {
        let document = self.document.clone();
        let access = self.access.clone();
        Box::pin(self.executor.spawn(async move {
            let _guard = access.lock();
            let fd = document.call(|env| {
                Ok(env
                    .call_method(document.object.as_obj(), "openRead", "()I", &[])?
                    .i()?)
            })?;
            ensure!(
                fd >= 0,
                "document provider returned an invalid file descriptor"
            );
            // SAFETY: openRead transfers a fresh descriptor using ParcelFileDescriptor.detachFd.
            let mut file = unsafe { File::from_raw_fd(fd) };
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        }))
    }

    fn can_write(&self) -> bool {
        self.writable
    }

    fn write(&self, contents: Vec<u8>) -> LocalBoxFuture<'static, Result<()>> {
        if !self.writable {
            return Box::pin(async { anyhow::bail!("file handle is read-only") });
        }
        let document = self.document.clone();
        let access = self.access.clone();
        Box::pin(self.executor.spawn(async move {
            let _guard = access.lock();
            let object = document.call(|env| {
                let stream = env
                    .call_method(
                        document.object.as_obj(),
                        "openWrite",
                        "()Ljava/io/OutputStream;",
                        &[],
                    )?
                    .l()?;
                Ok(env.new_global_ref(stream)?)
            })?;
            let stream = Document {
                vm: document.vm.clone(),
                object,
            };
            let result = stream.call(|env| {
                for chunk in contents.chunks(1024 * 1024) {
                    let bytes = env.byte_array_from_slice(chunk)?;
                    env.call_method(
                        stream.object.as_obj(),
                        "write",
                        "([B)V",
                        &[JValue::Object(bytes.as_ref())],
                    )?;
                    env.delete_local_ref(bytes)?;
                }
                env.call_method(stream.object.as_obj(), "flush", "()V", &[])?;
                Ok(())
            });
            // Keep the Java stream so provider close errors are reported, including after a failed write.
            let closed = stream.call(|env| {
                env.call_method(stream.object.as_obj(), "close", "()V", &[])?;
                Ok(())
            });
            result.and(closed)
        }))
    }
}
