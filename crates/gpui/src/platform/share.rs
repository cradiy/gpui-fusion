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
