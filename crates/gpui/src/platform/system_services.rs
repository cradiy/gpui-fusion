/// The application's default network, as reported by the operating system.
/// This is advisory state, not a guarantee that a particular server is reachable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkStatus {
    /// No default network is available to the application.
    Disconnected,
    /// A default network exists. Capabilities can be unknown during a transition.
    Connected {
        /// Whether the OS has validated Internet access on this network.
        internet_validated: Option<bool>,
        /// Whether the OS considers this network metered.
        metered: Option<bool>,
    },
}

/// A system settings page for the current application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppSettings {
    /// Application details, including its permissions.
    Application,
    /// The application's notification preferences.
    Notifications,
}

/// Why the operating system recommends releasing rebuildable memory.
/// These are advisory events, not a guarantee of notice before process termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTrimLevel {
    /// The application's UI is no longer visible. Release unused UI resources.
    UiHidden,
    /// The process is eligible for background reclamation. Reduce rebuildable caches.
    Background,
    /// The system reports memory pressure. Release unneeded allocations.
    Moderate,
    /// The system reports severe memory pressure. Release nonessential resources promptly.
    Critical,
}
