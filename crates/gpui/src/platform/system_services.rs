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
