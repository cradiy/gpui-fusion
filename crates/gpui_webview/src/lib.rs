//! Native rectangular browser views for GPUI.

mod backend;
mod view;

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

/// Browser lifecycle and navigation notifications.
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewStatus {
    Initializing,
    Ready,
    Closed,
    Failed(String),
}
