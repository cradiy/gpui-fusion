//! Single-line shaping with bounded font retention and grapheme-level fallback.
//!
//! Layouts own their selected fonts. Cache budgets count each shared byte buffer
//! once. Live layouts and supplied database data can outlive cache entries;
//! shaper metadata is not included in the byte budget.
//! Fallback ranks nominal character coverage by style, stretch, and weight in
//! database order. Language-specific preferences and emoji presentation matching
//! are not implemented. Font variations and custom OpenType features are not
//! applied; this is not a replacement for a platform text system.

use anyhow::{Context, Result, ensure};
use fontdb::ID;
use harfrust::{Direction, ShaperData, UnicodeBuffer};
use std::{
    collections::HashMap,
    ops::Range,
    path::PathBuf,
    sync::{Arc, Weak},
};
use unicode_bidi::BidiInfo;
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

pub use fontdb::{Database, Family, Query, Stretch, Style, Weight};

/// Limits for reusable data. Oversized font files remain usable without caching.
#[derive(Clone, Copy)]
pub struct CacheLimits {
    pub font_bytes: usize,
    pub font_entries: usize,
    pub fallback_entries: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            font_bytes: 32 * 1024 * 1024,
            font_entries: 32,
            fallback_entries: 512,
        }
    }
}

/// Fonts discovered by `fontdb` remain file references until selected for shaping.
pub struct TextSystem {
    db: Database,
    limits: CacheLimits,
    fonts: HashMap<ID, (Arc<FontData>, u64)>,
    font_bytes: usize,
    clock: u64,
    fallback: HashMap<(ID, String), ID>,
    mapped_files: bool,
    file_data: HashMap<PathBuf, Weak<dyn AsRef<[u8]> + Send + Sync>>,
}

struct FontData {
    bytes: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
    shaper: ShaperData,
    swash_offset: u32,
    swash_key: swash::CacheKey,
}

/// Glyph positions use pixels, with a downward-positive Y axis and UTF-8 clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub cluster: usize,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

pub struct GlyphRun {
    pub range: Range<usize>,
    pub rtl: bool,
    pub font_id: ID,
    pub glyphs: Vec<Glyph>,
    font: Arc<FontData>,
}

/// Runs and glyphs are ordered visually from left to right.
pub struct Line {
    pub runs: Vec<GlyphRun>,
    pub width: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheUsage {
    pub font_bytes: usize,
    pub font_entries: usize,
    pub fallback_entries: usize,
}

impl TextSystem {
    /// Shares supplied byte buffers and copies unshared file-backed fonts on first use.
    pub fn new(db: Database, limits: CacheLimits) -> Self {
        Self {
            db,
            limits,
            fonts: HashMap::new(),
            font_bytes: 0,
            clock: 0,
            fallback: HashMap::new(),
            mapped_files: false,
            file_data: HashMap::new(),
        }
    }

    /// Shares read-only file mappings instead of copying selected font files.
    ///
    /// # Safety
    /// File-backed fonts in `db` must not be modified in place or truncated while
    /// this system or any layout produced by it is alive. Replacing a file by
    /// renaming a different file over its path does not modify an existing mapping.
    pub unsafe fn new_with_mapped_fonts(db: Database, limits: CacheLimits) -> Self {
        Self {
            mapped_files: true,
            ..Self::new(db, limits)
        }
    }

    /// Mutations invalidate fallback decisions; existing layouts remain usable.
    pub fn add_font_data(&mut self, bytes: Vec<u8>) {
        self.db.load_font_data(bytes);
        self.fallback.clear();
    }

    pub fn cache_usage(&self) -> CacheUsage {
        CacheUsage {
            font_bytes: self.font_bytes,
            font_entries: self.fonts.len(),
            fallback_entries: self.fallback.len(),
        }
    }

    pub fn clear_caches(&mut self) {
        self.fonts.clear();
        self.font_bytes = 0;
        self.fallback.clear();
        self.file_data.retain(|_, data| data.strong_count() > 0);
    }

    fn font(&mut self, id: ID) -> Result<Arc<FontData>> {
        self.clock = self.clock.saturating_add(1);
        if let Some((font, stamp)) = self.fonts.get_mut(&id) {
            *stamp = self.clock;
            return Ok(font.clone());
        }
        let (source, index) = self.db.face_source(id).context("Missing font face")?;
        self.file_data.retain(|_, data| data.strong_count() > 0);
        let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = match source {
            fontdb::Source::Binary(data) | fontdb::Source::SharedFile(_, data) => data,
            fontdb::Source::File(path) => {
                if let Some(data) = self.file_data.get(&path).and_then(Weak::upgrade) {
                    data
                } else {
                    let data = if self.mapped_files {
                        // SAFETY: new_with_mapped_fonts requires file contents to remain
                        // immutable for the lifetimes of the system and its layouts.
                        let mapped = unsafe { self.db.make_shared_face_data(id) };
                        self.db.make_face_data_unshared(id);
                        mapped.context("Cannot map selected font")?.0
                    } else {
                        Arc::new(std::fs::read(&path).context("Cannot read selected font")?)
                            as Arc<dyn AsRef<[u8]> + Send + Sync>
                    };
                    self.file_data.insert(path, Arc::downgrade(&data));
                    data
                }
            }
        };
        let face = harfrust::FontRef::from_index(bytes.as_ref().as_ref(), index)?;
        let shaper = ShaperData::new(&face);
        let swash = swash::FontRef::from_index(bytes.as_ref().as_ref(), index as usize)
            .context("Invalid raster font")?;
        let (swash_offset, swash_key) = (swash.offset, swash.key);
        let font = Arc::new(FontData {
            bytes,
            index,
            shaper,
            swash_offset,
            swash_key,
        });
        let size = font.bytes.as_ref().as_ref().len();
        if size <= self.limits.font_bytes && self.limits.font_entries > 0 {
            loop {
                let already_cached = self
                    .fonts
                    .values()
                    .any(|(cached, _)| Arc::ptr_eq(&cached.bytes, &font.bytes));
                let additional = if already_cached { 0 } else { size };
                if self.fonts.len() < self.limits.font_entries
                    && self.font_bytes <= self.limits.font_bytes - additional
                {
                    self.font_bytes += additional;
                    break;
                }
                let oldest = *self
                    .fonts
                    .iter()
                    .min_by_key(|(_, (_, stamp))| stamp)
                    .unwrap()
                    .0;
                let removed = self.fonts.remove(&oldest).unwrap().0;
                if !self
                    .fonts
                    .values()
                    .any(|(cached, _)| Arc::ptr_eq(&cached.bytes, &removed.bytes))
                {
                    self.font_bytes -= removed.bytes.as_ref().as_ref().len();
                }
            }
            self.fonts.insert(id, (font.clone(), self.clock));
        }
        Ok(font)
    }

    fn supports(&self, id: ID, grapheme: &str) -> bool {
        let supports = |bytes: &[u8], index| {
            swash::FontRef::from_index(bytes, index as usize).is_some_and(|font| {
                let cmap = font.charmap();
                grapheme
                    .chars()
                    .all(|ch| is_ignorable(ch) || cmap.map(ch) != 0)
            })
        };
        if let Some((font, _)) = self.fonts.get(&id) {
            return supports(font.bytes.as_ref().as_ref(), font.index);
        }
        if let Some((fontdb::Source::File(path), index)) = self.db.face_source(id)
            && let Some(bytes) = self.file_data.get(&path).and_then(Weak::upgrade)
        {
            return supports(bytes.as_ref().as_ref(), index);
        }
        self.db.with_face_data(id, supports).unwrap_or(false)
    }

    fn select(&mut self, primary: ID, grapheme: &str) -> ID {
        let key = (primary, grapheme.to_owned());
        if let Some(&id) = self.fallback.get(&key) {
            return id;
        }
        let id = if self.supports(primary, grapheme) {
            primary
        } else {
            let primary_face = self.db.face(primary).unwrap();
            let mut candidates: Vec<_> = self
                .db
                .faces()
                .filter(|face| face.id != primary)
                .map(|face| {
                    (
                        (
                            face.style != primary_face.style,
                            face.stretch
                                .to_number()
                                .abs_diff(primary_face.stretch.to_number()),
                            face.weight.0.abs_diff(primary_face.weight.0),
                        ),
                        face.id,
                    )
                })
                .collect();
            candidates.sort_by_key(|&(score, _)| score);
            candidates
                .into_iter()
                .find_map(|(_, id)| self.supports(id, grapheme).then_some(id))
                .unwrap_or(primary)
        };
        // Bound key sizes as well as entry count for long combining sequences.
        if grapheme.len() <= 128 && self.limits.fallback_entries > 0 {
            if self.fallback.len() >= self.limits.fallback_entries {
                self.fallback.clear();
            }
            self.fallback.insert(key, id);
        }
        id
    }

    /// Shapes one paragraph with static font selection and default OpenType features.
    /// Tabs and paragraph separators require caller-side layout and are rejected.
    pub fn shape(&mut self, text: &str, query: &Query<'_>, size: f32) -> Result<Line> {
        ensure!(
            size.is_finite() && size > 0.,
            "Font size must be positive and finite"
        );
        ensure!(
            text.len() <= u32::MAX as usize,
            "Text exceeds shaping cluster range"
        );
        ensure!(
            !text
                .chars()
                .any(|ch| matches!(ch, '\t' | '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')),
            "Expected one line without tabs"
        );
        if text.is_empty() {
            return Ok(Line {
                runs: Vec::new(),
                width: 0.,
            });
        }
        let primary = self.db.query(query).context("No matching primary font")?;
        let bidi = BidiInfo::new(text, None);
        let para = bidi.paragraphs.first().context("Missing paragraph")?;
        let (levels, visual_runs) = bidi.visual_runs(para, para.range.clone());
        let mut result = Line {
            runs: Vec::new(),
            width: 0.,
        };
        let mut selected = HashMap::new();
        for range in visual_runs {
            let rtl = levels[range.start].is_rtl();
            let mut pieces: Vec<(Range<usize>, ID, Script)> = Vec::new();
            let mut script = Script::Common;
            for (offset, grapheme) in text[range.clone()].grapheme_indices(true) {
                let id = self.select(primary, grapheme);
                let strong = grapheme
                    .chars()
                    .map(|c| c.script())
                    .find(|s| !matches!(s, Script::Common | Script::Inherited));
                if let Some(strong) = strong {
                    script = strong;
                }
                let start = range.start + offset;
                if let Some((span, last_id, last_script)) = pieces.last_mut()
                    && *last_id == id
                    && (*last_script == script || *last_script == Script::Common)
                {
                    span.end = start + grapheme.len();
                    *last_script = script;
                } else {
                    pieces.push((start..start + grapheme.len(), id, script));
                }
            }
            if rtl {
                pieces.reverse();
            }
            for (range, id, script) in pieces {
                let font = match selected.get(&id) {
                    Some(font) => Arc::clone(font),
                    None => {
                        let font = self.font(id)?;
                        selected.insert(id, font.clone());
                        font
                    }
                };
                let face = harfrust::FontRef::from_index(font.bytes.as_ref().as_ref(), font.index)?;
                let shaper = font.shaper.shaper(&face).build();
                let scale = size / shaper.units_per_em() as f32;
                let mut buffer = UnicodeBuffer::new();
                buffer.set_direction(if rtl {
                    Direction::RightToLeft
                } else {
                    Direction::LeftToRight
                });
                buffer.set_script(script.short_name().parse().unwrap());
                buffer.set_pre_context(&text[..range.start]);
                buffer.set_post_context(&text[range.end..]);
                for (offset, ch) in text[range.clone()].char_indices() {
                    buffer.add(ch, (range.start + offset) as u32);
                }
                buffer.guess_segment_properties();
                let shaped = shaper.shape(buffer, &[]);
                let mut glyphs = Vec::with_capacity(shaped.len());
                for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                    let advance = pos.x_advance as f32 * scale;
                    glyphs.push(Glyph {
                        id: info.glyph_id,
                        cluster: info.cluster as usize,
                        x: result.width + pos.x_offset as f32 * scale,
                        y: -pos.y_offset as f32 * scale,
                        advance,
                    });
                    result.width += advance;
                }
                result.runs.push(GlyphRun {
                    range,
                    rtl,
                    font_id: id,
                    glyphs,
                    font,
                });
            }
        }
        Ok(result)
    }
}

fn is_ignorable(ch: char) -> bool {
    matches!(ch, '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{2060}' | '\u{feff}'
        | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

/// Reuses rasterization scratch storage independently of font selection.
#[derive(Default)]
pub struct Rasterizer(swash::scale::ScaleContext);
impl Rasterizer {
    pub fn rasterize(
        &mut self,
        run: &GlyphRun,
        glyph: u32,
        size: f32,
    ) -> Result<Option<swash::scale::image::Image>> {
        ensure!(
            size.is_finite() && size > 0.,
            "Font size must be positive and finite"
        );
        let font = swash::FontRef {
            data: run.font.bytes.as_ref().as_ref(),
            offset: run.font.swash_offset,
            key: run.font.swash_key,
        };
        let mut scaler = self.0.builder(font).size(size).build();
        Ok(swash::scale::Render::new(&[
            swash::scale::Source::ColorOutline(0),
            swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            swash::scale::Source::Outline,
        ])
        .render(&mut scaler, glyph.try_into()?))
    }
}

#[cfg(test)]
mod tests;
