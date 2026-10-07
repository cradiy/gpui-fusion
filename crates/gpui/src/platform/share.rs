use crate::SelectedFile;

/// Content offered through the platform's system share interface.
#[derive(Clone, Debug, Default)]
pub struct ShareOptions {
    /// Plain text or a URL. May accompany files; receivers decide how to use it.
    pub text: Option<String>,
    /// Files to expose with temporary read access. Finish writing before sharing.
    pub files: Vec<SelectedFile>,
    /// Optional system chooser title. Platforms may ignore it.
    pub title: Option<String>,
}

/// Text and read-only files received from another application.
#[derive(Clone, Debug)]
pub struct ReceivedShare {
    /// Plain text accompanying the share, if supplied.
    pub text: Option<String>,
    /// Sender-declared MIME type; applications must validate content themselves.
    pub mime_type: Option<String>,
    /// Files available under the sender's temporary access grant.
    /// Retaining a handle does not extend that grant or copy the file.
    pub files: Vec<SelectedFile>,
}
