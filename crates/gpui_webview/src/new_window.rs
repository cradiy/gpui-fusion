use crate::Url;
use std::{cell::RefCell, fmt, rc::Rc};

pub(crate) enum NewWindow {
    #[cfg(all(feature = "servo", target_os = "linux"))]
    Servo(::servo::CreateNewWebViewRequest),
}

/// A request to create an auxiliary browser view, including its opener relationship.
/// Clones share one request. Dropping the last unhandled clone rejects it.
#[derive(Clone)]
pub struct WebViewWindowRequest {
    /// The parent view's current URL, not necessarily the initiating frame's origin.
    pub source: Option<Url>,
    inner: Rc<RefCell<Option<NewWindow>>>,
}

impl fmt::Debug for WebViewWindowRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebViewWindowRequest")
            .field("source", &self.source)
            .field("pending", &self.is_pending())
            .finish()
    }
}

impl WebViewWindowRequest {
    pub fn is_pending(&self) -> bool {
        self.inner.borrow().is_some()
    }

    pub fn cancel(&self) {
        self.inner.borrow_mut().take();
    }

    pub(crate) fn take(&self) -> anyhow::Result<NewWindow> {
        self.inner
            .borrow_mut()
            .take()
            .ok_or_else(|| anyhow::anyhow!("New window request has already been handled"))
    }

    #[cfg(all(feature = "servo", target_os = "linux"))]
    pub(crate) fn new(source: Option<Url>, request: NewWindow) -> Self {
        Self {
            source,
            inner: Rc::new(RefCell::new(Some(request))),
        }
    }
}
