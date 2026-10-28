//! Native rectangular browser views for GPUI.

mod backend;
mod dialog;
mod new_window;
mod view;

pub use dialog::{WebViewDialog, WebViewDialogKind};
pub use new_window::WebViewWindowRequest;
pub use url::Url;
pub use view::WebView;

/// Browser engine selected when a view is created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WebViewBackend {
    /// Requires the `servo` feature and a Linux Wayland window.
    Servo,
}

impl WebViewBackend {
    /// Whether this build includes the backend. Window-system validation happens during initialization.
    pub fn is_available(self) -> bool {
        match self {
            Self::Servo => cfg!(all(feature = "servo", target_os = "linux")),
        }
    }
}

/// Configuration for an individual browser view.
#[derive(Clone, Debug)]
pub struct WebViewOptions {
    pub backend: WebViewBackend,
    pub url: Url,
}

impl WebViewOptions {
    pub fn new(backend: WebViewBackend, url: Url) -> Self {
        Self { backend, url }
    }
}

/// Severity or operation associated with a page console message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WebViewConsoleLevel {
    Log,
    Debug,
    Info,
    Warn,
    Error,
    Trace,
    Dir,
}

/// Text reported by the page console or an uncaught script error.
#[derive(Clone, Debug)]
pub struct WebViewConsoleMessage {
    pub level: WebViewConsoleLevel,
    pub message: String,
}

/// Browser lifecycle, navigation, and page notifications.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum WebViewEvent {
    Ready,
    Closed,
    LoadStarted,
    LoadFinished,
    TitleChanged(Option<String>),
    UrlChanged(Url),
    Failed(String),
    DialogRequested(WebViewDialog),
    NewWindowRequested(WebViewWindowRequest),
    ConsoleMessage(WebViewConsoleMessage),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewStatus {
    Initializing,
    Ready,
    Closed,
    Failed(String),
}
