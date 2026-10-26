use crate::Url;
use std::{cell::RefCell, fmt, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewDialogKind {
    Alert,
    Confirm,
    Prompt { default_value: String },
}

type Reply = Box<dyn FnOnce(Option<String>)>;

struct Response(RefCell<Option<Reply>>);
impl Response {
    fn resolve(&self, value: Option<String>) -> bool {
        let reply = self.0.borrow_mut().take();
        if let Some(reply) = reply {
            reply(value);
            true
        } else {
            false
        }
    }
}
impl Drop for Response {
    fn drop(&mut self) {
        if let Some(reply) = self.0.get_mut().take() {
            reply(None);
        }
    }
}

/// A page-controlled dialog. Display its source separately from trusted application UI.
/// Clones share one response; dropping the last unresolved clone cancels the request.
#[derive(Clone)]
pub struct WebViewDialog {
    pub kind: WebViewDialogKind,
    pub message: String,
    /// The top-level page URL, which may differ from the initiating frame's origin.
    pub source: Option<Url>,
    response: Rc<Response>,
}
impl fmt::Debug for WebViewDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebViewDialog")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("source", &self.source)
            .field("pending", &self.is_pending())
            .finish()
    }
}
impl WebViewDialog {
    pub fn is_pending(&self) -> bool {
        self.response.0.borrow().is_some()
    }
    /// Accept the dialog, using the default value for a text prompt.
    /// Returns false if the request has already been answered or invalidated.
    pub fn accept(&self) -> bool {
        let value = match &self.kind {
            WebViewDialogKind::Prompt { default_value } => default_value.clone(),
            _ => String::new(),
        };
        self.response.resolve(Some(value))
    }
    /// Submit a text prompt. Other dialog kinds are left unchanged and return false.
    pub fn submit(&self, value: impl Into<String>) -> bool {
        matches!(self.kind, WebViewDialogKind::Prompt { .. })
            && self.response.resolve(Some(value.into()))
    }
    pub fn cancel(&self) -> bool {
        self.response.resolve(None)
    }
    #[cfg(all(feature = "servo", target_os = "linux"))]
    pub(crate) fn new(
        kind: WebViewDialogKind,
        message: String,
        source: Option<Url>,
        reply: Reply,
    ) -> Self {
        Self {
            kind,
            message,
            source,
            response: Rc::new(Response(RefCell::new(Some(reply)))),
        }
    }
    #[cfg(all(feature = "servo", target_os = "linux"))]
    pub(crate) fn cancellation(&self) -> DialogCancellation {
        DialogCancellation(Rc::downgrade(&self.response))
    }
}

#[cfg(all(feature = "servo", target_os = "linux"))]
pub(crate) struct DialogCancellation(std::rc::Weak<Response>);
#[cfg(all(feature = "servo", target_os = "linux"))]
impl DialogCancellation {
    pub fn is_pending(&self) -> bool {
        self.0
            .upgrade()
            .is_some_and(|response| response.0.borrow().is_some())
    }
}
#[cfg(all(feature = "servo", target_os = "linux"))]
impl Drop for DialogCancellation {
    fn drop(&mut self) {
        if let Some(response) = self.0.upgrade() {
            response.resolve(None);
        }
    }
}
