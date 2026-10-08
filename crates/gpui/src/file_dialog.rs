use std::path::PathBuf;

/// Options shared by platform file pickers.
#[derive(Clone, Debug, Default)]
pub struct FilePromptOptions {
    /// Allow more than one file to be selected.
    pub multiple: bool,
    /// Request handles that allow replacing file contents. Unsupported platforms return an error.
    pub writable: bool,
    /// Accepted MIME types, combined as alternatives. Empty or `*/*` allows all files.
    /// Examples: `image/*`, `application/pdf`. Filters guide selection, not content validation.
    pub mime_types: Vec<String>,
}

impl FilePromptOptions {
    /// Validates and normalizes the MIME filters for a platform picker.
    pub fn normalized_mime_types(&self) -> anyhow::Result<Vec<String>> {
        let token = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'+-.^_`|~".contains(&b))
        };
        let mut result = Vec::new();
        for mime in &self.mime_types {
            let (kind, subtype) = mime
                .split_once('/')
                .ok_or_else(|| anyhow::anyhow!("invalid MIME filter: {mime}"))?;
            anyhow::ensure!(
                (token(kind) && (token(subtype) || subtype == "*")) || mime == "*/*",
                "invalid MIME filter: {mime}"
            );
            result.push(mime.to_ascii_lowercase());
        }
        if result.iter().any(|mime| mime == "*/*") {
            return Ok(Vec::new());
        }
        result.sort();
        result.dedup();
        Ok(result)
    }

    /// Maps MIME filters to known extensions for extension-based native pickers.
    /// Unknown MIME types return an error instead of silently disabling filtering.
    pub fn filter_extensions(mime_types: &[String]) -> anyhow::Result<Vec<&'static str>> {
        let mut extensions = Vec::new();
        for mime in mime_types {
            let known = mime_guess::get_mime_extensions_str(mime).ok_or_else(|| {
                anyhow::anyhow!("no file extensions are known for MIME type {mime}")
            })?;
            extensions.extend_from_slice(known);
        }
        extensions.sort_unstable();
        extensions.dedup();
        Ok(extensions)
    }
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
