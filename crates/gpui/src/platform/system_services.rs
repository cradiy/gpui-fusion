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

/// An output exposed by the operating system's media routing framework.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioOutputDevice {
    /// Opaque identifier valid while this device remains available.
    pub id: String,
    /// User-visible device name supplied by the system.
    pub name: String,
    /// The system's classification of this output.
    pub kind: AudioOutputKind,
    /// Whether media is currently routed to this device.
    pub selected: bool,
}

/// Physical media output categories; remote protocols require their own providers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioOutputKind {
    /// Built-in speaker or earpiece.
    BuiltIn,
    /// Analog wired headphones or headset.
    Headphones,
    /// Bluetooth audio, including hearing aids and LE Audio.
    Bluetooth,
    /// USB audio device or headset.
    Usb,
    /// HDMI or digital display output.
    Hdmi,
    /// An output outside the known categories.
    Other,
}

/// Authoritative media routes visible to the application.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioOutputState {
    /// Available outputs, including the selected output when the OS exposes it.
    pub devices: Vec<AudioOutputDevice>,
    /// Additional user authorization is needed for the complete device list.
    /// Android requires nearby-device permissions for Bluetooth routing.
    pub authorization_required: bool,
}
