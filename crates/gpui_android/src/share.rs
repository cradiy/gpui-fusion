use crate::bridge::Host;
use anyhow::Result;
use futures::{StreamExt, channel::mpsc};
use gpui::{ForegroundExecutor, ReceivedShare, Task};
use jni::{JavaVM, objects::GlobalRef};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::Arc,
};

pub(crate) struct IncomingShare {
    pub text: Option<String>,
    pub mime_type: Option<String>,
    pub documents: Vec<GlobalRef>,
    pub vm: Arc<JavaVM>,
}

type Callback = Box<dyn FnMut(Result<ReceivedShare>)>;

pub(crate) struct ShareReceiver {
    host: Arc<Host>,
    sender: mpsc::UnboundedSender<Result<IncomingShare>>,
    task: RefCell<Option<Task<()>>>,
    ready: RefCell<VecDeque<Result<ReceivedShare>>>,
    callback: RefCell<Option<Callback>>,
    closed: Cell<bool>,
}

impl ShareReceiver {
    pub fn new(host: Arc<Host>, foreground: &ForegroundExecutor) -> Rc<Self> {
        let (sender, mut receiver) = mpsc::unbounded::<Result<IncomingShare>>();
        let state = Rc::new(Self {
            host,
            sender,
            task: RefCell::default(),
            ready: RefCell::default(),
            callback: RefCell::default(),
            closed: Cell::new(false),
        });
        let weak = Rc::downgrade(&state);
        *state.task.borrow_mut() = Some(foreground.spawn(async move {
            while let Some(incoming) = receiver.next().await {
                let executor = crate::dispatcher::io_executor();
                let files_executor = executor.clone();
                let result = executor
                    .run(move || {
                        let incoming = incoming?;
                        let files = incoming
                            .documents
                            .into_iter()
                            .map(|object| {
                                crate::file::selected_file(
                                    incoming.vm.clone(),
                                    object,
                                    files_executor.clone(),
                                )
                            })
                            .collect::<Result<Vec<_>>>()?;
                        Ok(ReceivedShare {
                            text: incoming.text,
                            mime_type: incoming.mime_type,
                            files,
                        })
                    })
                    .await;
                let Some(state) = weak.upgrade() else { break };
                state.ready.borrow_mut().push_back(result);
                state.host.request_frame();
            }
        }));
        state
    }

    pub fn receive(&self, share: Result<IncomingShare>) {
        let _ = self.sender.unbounded_send(share);
    }

    pub fn set_callback(&self, callback: Callback) {
        *self.callback.borrow_mut() = Some(callback);
        if !self.ready.borrow().is_empty() {
            self.host.request_frame();
        }
    }

    pub fn dispatch(&self) {
        let Some(mut callback) = self.callback.borrow_mut().take() else {
            return;
        };
        let ready = std::mem::take(&mut *self.ready.borrow_mut());
        for share in ready {
            if self.closed.get() {
                return;
            }
            callback(share);
        }
        if !self.closed.get() {
            self.callback.borrow_mut().get_or_insert(callback);
        }
    }

    pub fn close(&self) {
        self.closed.set(true);
        self.sender.close_channel();
        self.task.borrow_mut().take();
        self.ready.borrow_mut().clear();
        self.callback.borrow_mut().take();
    }
}
