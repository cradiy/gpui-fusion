use std::path::PathBuf;

/// Options shared by platform file pickers.
#[derive(Clone, Debug, Default)]
pub struct FilePromptOptions {
    /// Allow more than one file to be selected.
    pub multiple: bool,
    /// Request handles that allow replacing file contents. Unsupported platforms return an error.
    pub writable: bool,
}

/// Options for choosing a writable destination through a system save dialog.
#[derive(Clone, Debug)]
pub struct FileSaveOptions {
    /// Suggested filename, including its extension.
    pub suggested_name: String,
    /// Content MIME type. Used by Android document providers.
    pub mime_type: String,
    /// Initial directory on desktop. Ignored by URI-based document pickers.
    pub directory: Option<PathBuf>,
}

impl Default for FileSaveOptions {
    fn default() -> Self {
        Self {
            suggested_name: "untitled".into(),
            mime_type: "application/octet-stream".into(),
            directory: None,
        }
    }
}
