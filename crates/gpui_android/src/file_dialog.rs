use crate::bridge::Host;
use anyhow::{Result, anyhow, ensure};
use futures::channel::oneshot;
use gpui::{
    BackgroundExecutor, FilePromptOptions, FileSaveOptions, ForegroundExecutor, SelectedFile,
};
use jni::{JavaVM, objects::GlobalRef};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

enum Selected {
    Files(Vec<SelectedFile>),
    Directory(gpui::gpui_io::LocationHandle),
}
type Selection = Result<Option<Selected>>;
type Pending = (u64, bool, oneshot::Sender<Selection>);

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

    pub fn prompt(
        &self,
        options: FilePromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<SelectedFile>>>> {
        let selection = self.prompt_with(false, |token| {
            let mime_types = options.normalized_mime_types()?;
            self.host
                .request_files(token, options.multiple, options.writable, &mime_types)
        });
        let (tx, rx) = oneshot::channel();
        self.foreground
            .spawn(async move {
                let result = async {
                    match selection.await?? {
                        Some(Selected::Files(files)) => Ok(Some(files)),
                        None => Ok(None),
                        _ => Err(anyhow!("unexpected directory selection")),
                    }
                }
                .await;
                let _ = tx.send(result);
            })
            .detach();
        rx
    }

    pub fn prompt_directory(
        &self,
    ) -> oneshot::Receiver<Result<Option<gpui::gpui_io::LocationHandle>>> {
        let selection = self.prompt_with(true, |token| self.host.request_directory(token));
        let (tx, rx) = oneshot::channel();
        self.foreground
            .spawn(async move {
                let result = async {
                    match selection.await?? {
                        Some(Selected::Directory(directory)) => Ok(Some(directory)),
                        None => Ok(None),
                        _ => Err(anyhow!("unexpected file selection")),
                    }
                }
                .await;
                let _ = tx.send(result);
            })
            .detach();
        rx
    }

    pub fn prompt_save(
        &self,
        options: FileSaveOptions,
    ) -> oneshot::Receiver<Result<Option<SelectedFile>>> {
        let selection =
            self.prompt_with(false, |token| self.host.request_file_save(token, &options));
        let (tx, rx) = oneshot::channel();
        self.foreground
            .spawn(async move {
                let result = async {
                    let Some(selected) = selection.await?? else {
                        return Ok(None);
                    };
                    let Selected::Files(mut files) = selected else {
                        return Err(anyhow!("unexpected directory selection"));
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

    fn prompt_with(
        &self,
        directory: bool,
        request: impl FnOnce(u64) -> Result<()>,
    ) -> oneshot::Receiver<Selection> {
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
        *self.pending.borrow_mut() = Some((token, directory, tx));
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
        let directory = match self.pending.borrow().as_ref() {
            Some((id, directory, _)) if *id == token => *directory,
            _ => return,
        };
        let objects = match result {
            Ok(Some(objects)) => objects,
            other => {
                self.finish(token, other.map(|_| None));
                return;
            }
        };
        let executor = crate::dispatcher::io_executor();
        let task = self.background.spawn(async move {
            if directory {
                ensure!(
                    objects.len() == 1,
                    "directory picker must return one directory"
                );
                let object = objects.into_iter().next().unwrap();
                return crate::directory::selected_directory(vm, object, executor)
                    .map(Selected::Directory)
                    .map(Some);
            }
            objects
                .into_iter()
                .map(|object| crate::file::selected_file(vm.clone(), object, executor.clone()))
                .collect::<Result<Vec<_>>>()
                .map(Selected::Files)
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
            if slot.as_ref().is_some_and(|(id, _, _)| *id == token) {
                slot.take()
            } else {
                None
            }
        };
        if let Some((_, _, tx)) = pending {
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
        if let Some((_, _, tx)) = pending {
            let _ = tx.send(Err(anyhow!("Android file selection session closed")));
        }
    }
}
