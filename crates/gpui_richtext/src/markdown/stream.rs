use crate::{Projection, Registry, RichText};
use anyhow::{Result, bail};
use std::sync::Arc;

/// Buffered Markdown input with explicitly published immutable display snapshots.
///
/// Append decoded UTF-8 chunks with `push_str`, then call `flush` at the desired
/// UI cadence. A flush reparses the complete source so late reference definitions
/// and unfinished block syntax retain the same semantics as `markdown::parse`.
/// No text or closing delimiters are synthesized. Transport and scrolling belong
/// to the application. Failed flushes leave the last published snapshot intact.
pub struct MarkdownStream {
    registry: Registry,
    source: String,
    projection: Arc<Projection>,
    front_matter: Option<super::FrontMatter>,
    dirty: bool,
    finished: bool,
    revision: u64,
}

impl MarkdownStream {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            source: String::new(),
            projection: Arc::default(),
            front_matter: None,
            dirty: false,
            finished: false,
            revision: 0,
        }
    }

    pub fn push_str(&mut self, chunk: &str) -> Result<()> {
        if self.finished {
            bail!("cannot append to a finished Markdown stream");
        }
        if !chunk.is_empty() {
            self.source.push_str(chunk);
            self.dirty = true;
        }
        Ok(())
    }

    /// Publish accumulated input. Calling this without new input reuses the snapshot.
    pub fn flush(&mut self) -> Result<Arc<Projection>> {
        if self.dirty {
            let parsed = super::parse_with_metadata(&self.source)?;
            let projection = Arc::new(self.registry.compile(&parsed.document)?);
            self.projection = projection;
            self.front_matter = parsed.front_matter;
            self.dirty = false;
        }
        Ok(self.projection.clone())
    }

    /// Publish the final snapshot and reject further input until `clear`.
    pub fn finish(&mut self) -> Result<Arc<Projection>> {
        let projection = self.flush()?;
        self.finished = true;
        Ok(projection)
    }

    /// Metadata from the latest successful flush, independent of the body snapshot.
    pub fn front_matter(&self) -> Option<&super::FrontMatter> {
        self.front_matter.as_ref()
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    pub fn projection(&self) -> &Arc<Projection> {
        &self.projection
    }

    /// Build a view retaining selections whose text prefix survives an update.
    pub fn view(&self, id: impl Into<gpui::ElementId>) -> RichText {
        let id = gpui::ElementId::NamedChild(
            std::sync::Arc::new(id.into()),
            format!("stream-{}", self.revision).into(),
        );
        RichText::new(id, self.projection.clone())
            .preserve_selection_on_update(true)
            .selection_revision(self.revision)
    }

    pub fn clear(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        #[cfg(feature = "syntax-highlighting")]
        self.registry.clear_highlight_cache();
        self.source.clear();
        self.front_matter = None;
        self.projection = Arc::default();
        self.dirty = false;
        self.finished = false;
    }
}
