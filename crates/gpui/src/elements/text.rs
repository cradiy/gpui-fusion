use crate::{
    ActiveTooltip, AnyView, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Size, TextOverflow,
    TextRun, TextStyle, TooltipId, TruncateFrom, WhiteSpace, Window, WrappedLine,
    WrappedLineLayout, register_tooltip_mouse_handlers, set_tooltip_on_window,
};
use anyhow::Context as _;
use gpui_util::ResultExt;
use itertools::Itertools;
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    mem,
    ops::{Deref, DerefMut, Range},
    rc::Rc,
    sync::Arc,
};

mod selection;
use selection::TextSelection;

/// An [`Element`] that renders text.
///
/// In general, [`Text`] objects should be created via the [`text!`](macro@crate::text) macro:
/// ```rust
/// # use gpui::*;
/// # fn render() -> impl IntoElement {
/// div().child(text!("hello"))
/// # }
/// ```
/// ## IDs and Accessibility
///
/// [`Text`] elements have an ID. This ID is primarily used to produce nodes in
/// the accessibility tree, which allows the text to be visible to screen
/// readers and other assistive technologies.
///
/// This ID is stable across frames. If the same text, with the same ID, is
/// present in two consecutive frames, no updates are reported to the screen
/// reader. If the text changes, but the ID stays the same, then the screen
/// reader will be notified that a text node's content has changed. **However**,
/// if the ID changes, then the screen reader will be notified that a node has
/// been removed, and a new node has been added.
///
/// When using the [`text!`](macro@crate::text) macro, each invocation of the macro will get a
/// unique ID, derived from its position in the source code (filename, line, and
/// column). For example:
/// ```rust
/// # use gpui::*;
/// let x = text!("hello");
/// let y = text!("hello");
/// // not equal, because different `text!` invocations produced them
/// assert_ne!(x.id(), y.id());
///
/// fn make_text(s: &str) -> Text { text!(s) }
/// let x = make_text("hello");
/// let y = make_text("hello");
/// // equal, because the same `text!` invocation produced them
/// assert_eq!(x.id(), y.id());
/// ```
/// When the contents of an invocation of [`text!`](macro@crate::text) do not change, this
/// distinction is less relevant (with the caveat that you still need to take
/// care to ensure that duplicate IDs do not appear).
///
/// However, when a [`text!`](macro@crate::text) invocation's argument *does* change, you should
/// consider whether this change should be reported as a node "updating its
/// contents", or an old node being destroyed and a new node being created.
#[derive(Debug, Clone)]
pub struct Text {
    id: Option<ElementId>,
    text: SharedString,
}

impl Text {
    /// Create a new [`Text`] element with a specific ID.
    ///
    /// If you want a unique ID to be assigned automatically, use the [`text!`](macro@crate::text)
    /// macro. The docs for [`Text`] have more detail about choosing IDs.
    #[inline]
    pub const fn new(id: ElementId, text: SharedString) -> Self {
        Self { id: Some(id), text }
    }

    /// Create a new [`Text`] element that is inaccessible to screen readers.
    ///
    /// In order for text to be accessible to screen readers, it must have an ID
    /// provided. If you want text to be accessible, either use [`text!`](macro@crate::text) to have
    /// an ID automatically assigned, or use [`Text::new`] to manually assign an
    /// ID.
    ///
    /// This function is intended for use inside custom UI components, where
    /// accessible properties may be set on parent containers.
    #[inline]
    pub const fn new_inaccessible(text: SharedString) -> Self {
        Self { id: None, text }
    }

    /// The ID of this [`Text`] element.
    #[inline]
    pub const fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// Produce a new [`Text`] with the given `id`.
    pub fn with_id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The text that this [`Text`] element will display.
    #[inline]
    pub const fn text(&self) -> &SharedString {
        &self.text
    }
}

impl Deref for Text {
    type Target = SharedString;
    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.text
    }
}

/// Trivial hash function for the location information produced by the [`text!`](macro@crate::text)
/// macro. Not covered by semver guarantees. Performance is not particularly
/// significant because it's only used on small strings in const contexts.
#[doc(hidden)]
pub const fn __hash_text_macro_location_unstable_do_not_use(s: &'static str) -> u64 {
    const BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let bytes = s.as_bytes();
    let mut hash = BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(PRIME);
        i += 1;
    }
    hash
}

/// Create a new [`Text`] element.
///
/// ```rust
/// # use gpui::*;
/// let a = text!("hello");
/// let b = text!(id = "farewell-message", "hello");
///
/// ```
///
/// Text created with this macro is *accessible*. The macro generates an ID
/// based on the source location. See the docs for [`Text`] for a more in-depth
/// explanation of the significance of the ID of a [`Text`] element.
#[macro_export]
macro_rules! text {
    (id = $id:expr, $text:expr) => {{ $crate::Text::new($id.into(), $text.into()) }};
    ($text:expr) => {{
        const ID: &'static str = concat!(file!(), "/", line!(), ":", column!());
        const HASH: u64 = $crate::__hash_text_macro_location_unstable_do_not_use(ID);
        $crate::Text::new($crate::ElementId::Integer(HASH), $text.into())
    }};
}

impl IntoElement for Text {
    type Element = Self;
    #[inline]
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Text {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        if self.id.is_some() {
            Some(accesskit::Role::Label)
        } else {
            None
        }
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.text.to_string());
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        <SharedString as Element>::request_layout(&mut self.text, id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        <SharedString as Element>::prepaint(
            &mut self.text,
            id,
            inspector_id,
            bounds,
            request_layout,
            window,
            cx,
        )
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        <SharedString as Element>::paint(
            &mut self.text,
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}

impl Element for &'static str {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(
            SharedString::from(*self),
            None,
            TextLayoutOptions::default(),
            window,
            cx,
        );
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        text_layout.prepaint(bounds, self, _window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut TextLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        text_layout.paint(self, window, cx)
    }
}

impl IntoElement for &'static str {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl IntoElement for String {
    type Element = SharedString;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl IntoElement for Cow<'static, str> {
    type Element = SharedString;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl Element for SharedString {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(self.clone(), None, TextLayoutOptions::default(), window, cx);
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        text_layout.prepaint(bounds, self.as_ref(), _window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        text_layout.paint(self.as_ref(), window, cx)
    }
}

impl IntoElement for SharedString {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Renders text with runs of different styles.
///
/// Callers are responsible for setting the correct style for each run.
/// For text with a uniform style, you can usually avoid calling this constructor
/// and just pass text directly.
pub struct StyledText {
    inline_boxes: Vec<TextInlineBox>,
    text: SharedString,
    runs: Option<Vec<TextRun>>,
    delayed_highlights: Option<Vec<(Range<usize>, HighlightStyle)>>,
    delayed_font_family_overrides: Option<Vec<(Range<usize>, SharedString)>>,
    layout: TextLayout,
    line_top_spacing: Vec<(usize, Pixels)>,
    line_indents: Vec<TextLineIndent>,
    empty_line_heights: Vec<(usize, Pixels)>,
    line_typography: Vec<TextLineTypography>,
}

/// A single atomic inline object replacing a UTF-8 source range.
/// Source glyphs are hidden; callers paint the object separately at its baseline position.
#[derive(Clone, Debug)]
pub struct TextInlineBox {
    /// Nonempty UTF-8 byte range replaced by the object.
    pub range: Range<usize>,
    /// Width and total height reserved in logical pixels.
    pub size: Size<Pixels>,
    /// Distance from the object's top edge to the text baseline.
    pub baseline: Pixels,
}

/// Typography for a newline-delimited source line, including its soft wraps.
#[derive(Clone, Copy, Debug)]
pub struct TextLineTypography {
    /// UTF-8 byte offset of the first character in the source line.
    pub line_start: usize,
    /// Font size used for shaping and painting this line.
    pub font_size: Pixels,
    /// Height of each visual row, including wrapped continuations.
    pub line_height: Pixels,
}

/// A source-line gutter shared by its first and wrapped continuation rows.
#[derive(Clone, Copy, Debug)]
pub struct TextLineIndent {
    /// UTF-8 byte offset of the source line's first character.
    pub line_start: usize,
    /// Prefix bytes retained for copying but collapsed during layout. Callers
    /// must hide their glyphs and draw any marker separately.
    pub prefix_len: usize,
    /// Left inset reserved for every visual row of this source line.
    pub width: Pixels,
}

impl StyledText {
    /// Replace sorted, non-overlapping source ranges with atomic layout boxes.
    /// Invalid ranges or dimensions are ignored. Newlines inside an object do not break it.
    /// Text-overflow truncation uses the original source representation.
    pub fn with_inline_boxes(mut self, boxes: Vec<TextInlineBox>) -> Self {
        let mut end = 0;
        self.inline_boxes = boxes
            .into_iter()
            .filter(|object| {
                let valid = object.range.start >= end
                    && object.range.start < object.range.end
                    && self.text.get(object.range.clone()).is_some()
                    && [object.size.width, object.size.height, object.baseline]
                        .iter()
                        .all(|value| f32::from(*value).is_finite() && *value >= Pixels::ZERO)
                    && object.baseline <= object.size.height;
                if valid {
                    end = object.range.end;
                }
                valid
            })
            .collect();
        self
    }

    /// Construct a new styled text element from the given string.
    pub fn new(text: impl Into<SharedString>) -> Self {
        StyledText {
            inline_boxes: Vec::new(),
            text: text.into(),
            runs: None,
            delayed_highlights: None,
            delayed_font_family_overrides: None,
            layout: TextLayout::default(),
            line_top_spacing: Vec::new(),
            line_indents: Vec::new(),
            empty_line_heights: Vec::new(),
            line_typography: Vec::new(),
        }
    }

    /// Get the layout for this element. This can be used to map indices to pixels and vice versa.
    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }

    /// Reserve vertical space before source lines without adding text. Offsets
    /// must identify the start of a newline-delimited line. Invalid offsets are
    /// ignored; negative spacing is clamped to zero. Wrapped continuations do
    /// not repeat the spacing.
    pub fn with_line_top_spacing(mut self, spacing: Vec<(usize, Pixels)>) -> Self {
        self.line_top_spacing = spacing;
        self
    }

    /// Indent specified newline-delimited source lines, including soft wraps.
    /// Invalid line offsets, prefix boundaries, and widths are ignored.
    pub fn with_line_indents(mut self, indents: Vec<TextLineIndent>) -> Self {
        self.line_indents = indents;
        self
    }

    /// Override the height of selected empty source lines by UTF-8 start offset.
    /// Nonempty lines and nonpositive or nonfinite heights are ignored.
    pub fn with_empty_line_heights(mut self, heights: Vec<(usize, Pixels)>) -> Self {
        self.empty_line_heights = heights;
        self
    }

    /// Override source-line typography while retaining continuous selection.
    /// Invalid offsets and nonpositive or nonfinite metrics are ignored.
    /// Overrides do not apply when text-overflow truncation is active.
    pub fn with_line_typography(mut self, typography: Vec<TextLineTypography>) -> Self {
        self.line_typography = typography;
        self
    }

    /// Set the styling attributes for the given text, as well as
    /// as any ranges of text that have had their style customized.
    pub fn with_default_highlights(
        mut self,
        default_style: &TextStyle,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        debug_assert!(
            self.delayed_highlights.is_none(),
            "Can't use `with_default_highlights` and `with_highlights`"
        );
        let runs = Self::compute_runs(&self.text, default_style, highlights);
        self.with_runs(runs)
    }

    /// Set the styling attributes for the given text, as well as
    /// as any ranges of text that have had their style customized.
    pub fn with_highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        debug_assert!(
            self.runs.is_none(),
            "Can't use `with_highlights` and `with_default_highlights`"
        );
        self.delayed_highlights = Some(
            highlights
                .into_iter()
                .inspect(|(run, _)| {
                    debug_assert!(self.text.is_char_boundary(run.start));
                    debug_assert!(self.text.is_char_boundary(run.end));
                })
                .collect::<Vec<_>>(),
        );
        self
    }

    fn compute_runs(
        text: &str,
        default_style: &TextStyle,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Vec<TextRun> {
        let mut runs = Vec::new();
        let mut ix = 0;
        for (range, highlight) in highlights {
            if ix < range.start {
                debug_assert!(text.is_char_boundary(range.start));
                runs.push(default_style.clone().to_run(range.start - ix));
            }
            debug_assert!(text.is_char_boundary(range.end));
            runs.push(
                default_style
                    .clone()
                    .highlight(highlight)
                    .to_run(range.len()),
            );
            ix = range.end;
        }
        if ix < text.len() {
            runs.push(default_style.to_run(text.len() - ix));
        }
        runs
    }

    /// Override the font family for specific byte ranges of the text.
    ///
    /// This is resolved lazily at layout time, so the overrides are applied
    /// on top of the inherited text style from the parent element.
    /// Can be combined with [`with_highlights`](Self::with_highlights).
    ///
    /// The overrides must be sorted by range start and non-overlapping.
    /// Each override range must fall on character boundaries.
    pub fn with_font_family_overrides(
        mut self,
        overrides: impl IntoIterator<Item = (Range<usize>, SharedString)>,
    ) -> Self {
        self.delayed_font_family_overrides = Some(
            overrides
                .into_iter()
                .inspect(|(range, _)| {
                    debug_assert!(self.text.is_char_boundary(range.start));
                    debug_assert!(self.text.is_char_boundary(range.end));
                })
                .collect(),
        );
        self
    }

    fn apply_font_family_overrides(
        runs: &mut [TextRun],
        overrides: &[(Range<usize>, SharedString)],
    ) {
        let mut byte_offset = 0;
        let mut override_idx = 0;
        for run in runs.iter_mut() {
            let run_end = byte_offset + run.len;
            while override_idx < overrides.len() && overrides[override_idx].0.end <= byte_offset {
                override_idx += 1;
            }
            if override_idx < overrides.len() {
                let (ref range, ref family) = overrides[override_idx];
                if byte_offset >= range.start && run_end <= range.end {
                    run.font.family = family.clone();
                }
            }
            byte_offset = run_end;
        }
    }

    /// Set the text runs for this piece of text.
    pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self {
        let mut text = &*self.text;
        for run in &runs {
            text = text.get(run.len..).unwrap_or_else(|| {
                #[cfg(debug_assertions)]
                panic!("invalid text run. Text: '{text}', run: {run:?}");
                #[cfg(not(debug_assertions))]
                panic!("invalid text run");
            });
        }
        assert!(text.is_empty(), "invalid text run");
        self.runs = Some(runs);
        self
    }
}

impl Element for StyledText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let font_family_overrides = self.delayed_font_family_overrides.take();
        let mut runs = self.runs.take().or_else(|| {
            self.delayed_highlights.take().map(|delayed_highlights| {
                Self::compute_runs(&self.text, &window.text_style(), delayed_highlights)
            })
        });

        if let Some(ref overrides) = font_family_overrides {
            let runs =
                runs.get_or_insert_with(|| vec![window.text_style().to_run(self.text.len())]);
            Self::apply_font_family_overrides(runs, overrides);
        }

        if window.text_style().text_overflow.is_some() {
            self.inline_boxes.clear();
        }
        if !self.inline_boxes.is_empty() {
            let source = runs
                .take()
                .unwrap_or_else(|| vec![window.text_style().to_run(self.text.len())]);
            let mut hidden = Vec::new();
            let mut offset = 0;
            for run in source {
                let end = offset + run.len;
                let mut boundaries = vec![offset, end];
                for object in &self.inline_boxes {
                    if object.range.start > offset && object.range.start < end {
                        boundaries.push(object.range.start);
                    }
                    if object.range.end > offset && object.range.end < end {
                        boundaries.push(object.range.end);
                    }
                }
                boundaries.sort_unstable();
                boundaries.dedup();
                for pair in boundaries.windows(2) {
                    let mut part = run.clone();
                    part.len = pair[1] - pair[0];
                    if self
                        .inline_boxes
                        .iter()
                        .any(|object| object.range.contains(&pair[0]))
                    {
                        part.color = crate::transparent_black();
                        part.underline = None;
                        part.strikethrough = None;
                        part.background_color = None;
                    }
                    hidden.push(part);
                }
                offset = end;
            }
            runs = Some(hidden);
        }

        let layout_id = self.layout.layout(
            self.text.clone(),
            runs,
            TextLayoutOptions {
                inline_boxes: std::mem::take(&mut self.inline_boxes),
                line_top_spacing: std::mem::take(&mut self.line_top_spacing),
                line_indents: std::mem::take(&mut self.line_indents),
                empty_line_heights: std::mem::take(&mut self.empty_line_heights),
                line_typography: std::mem::take(&mut self.line_typography),
            },
            window,
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        self.layout.prepaint(bounds, &self.text, _window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.layout.paint(&self.text, window, cx)
    }
}

impl IntoElement for StyledText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The Layout for TextElement. This can be used to map indices to pixels and vice versa.
#[derive(Default, Clone)]
pub struct TextLayout(Rc<RefCell<Option<TextLayoutInner>>>);

struct TextLayoutInner {
    text: SharedString,
    len: usize,
    lines: SmallVec<[WrappedLine; 1]>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    size: Option<Size<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
    line_top_spacing: Vec<Pixels>,
    line_indents: Vec<Pixels>,
    line_heights: Vec<Pixels>,
}

#[derive(Default)]
struct TextLayoutOptions {
    inline_boxes: Vec<TextInlineBox>,
    line_top_spacing: Vec<(usize, Pixels)>,
    line_indents: Vec<TextLineIndent>,
    empty_line_heights: Vec<(usize, Pixels)>,
    line_typography: Vec<TextLineTypography>,
}

impl TextLayoutInner {
    fn height_for_line(&self, index: usize) -> Pixels {
        self.line_heights
            .get(index)
            .copied()
            .unwrap_or(self.line_height)
    }
}

impl TextLayout {
    fn layout(
        &self,
        text: SharedString,
        runs: Option<Vec<TextRun>>,
        options: TextLayoutOptions,
        window: &mut Window,
        _: &mut App,
    ) -> LayoutId {
        let TextLayoutOptions {
            inline_boxes,
            line_top_spacing,
            line_indents,
            empty_line_heights,
            line_typography,
        } = options;
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            text_style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );

        let runs = if let Some(runs) = runs {
            runs
        } else {
            vec![text_style.to_run(text.len())]
        };
        window.request_measured_layout(Default::default(), {
            let element_state = self.clone();

            move |known_dimensions, available_space, window, cx| {
                let wrap_width = if text_style.white_space == WhiteSpace::Normal {
                    known_dimensions.width.or(match available_space.width {
                        crate::AvailableSpace::Definite(x) => Some(x),
                        _ => None,
                    })
                } else {
                    None
                };

                let (truncate_width, truncation_affix, truncate_from) =
                    if let Some(text_overflow) = text_style.text_overflow.clone() {
                        let width = known_dimensions.width.or(match available_space.width {
                            crate::AvailableSpace::Definite(x) => match text_style.line_clamp {
                                Some(max_lines) => Some(x * max_lines),
                                None => Some(x),
                            },
                            _ => None,
                        });

                        match text_overflow {
                            TextOverflow::Truncate(s) => (width, s, TruncateFrom::End),
                            TextOverflow::TruncateStart(s) => (width, s, TruncateFrom::Start),
                            TextOverflow::TruncateMiddle(s) => (width, s, TruncateFrom::Middle),
                        }
                    } else {
                        (None, "".into(), TruncateFrom::End)
                    };

                // Only use cached layout if:
                // 1. We have a cached size
                // 2. wrap_width matches (or both are None)
                // 3. truncate_width is None (if truncate_width is Some, we need to re-layout
                //    because the previous layout may have been computed without truncation)
                if let Some(text_layout) = element_state.0.borrow().as_ref()
                    && let Some(size) = text_layout.size
                    && (wrap_width.is_none() || wrap_width == text_layout.wrap_width)
                    && truncate_width.is_none()
                {
                    return size;
                }

                let mut line_wrapper = cx.text_system().line_wrapper(text_style.font(), font_size);
                let (text, runs) = if truncate_width.is_some() {
                    if let Some(max_lines) = text_style.line_clamp
                        && let Some(wrap_width) = wrap_width
                    {
                        line_wrapper.truncate_wrapped_line(
                            text.clone(),
                            wrap_width,
                            max_lines,
                            &truncation_affix,
                            &runs,
                            truncate_from,
                        )
                    } else {
                        line_wrapper.truncate_line(
                            text.clone(),
                            truncate_width.unwrap_or(Pixels::MAX),
                            &truncation_affix,
                            &runs,
                            truncate_from,
                        )
                    }
                } else {
                    (text.clone(), Cow::Borrowed(&*runs))
                };
                let len = text.len();

                let typography: std::collections::HashMap<_, _> = line_typography
                    .iter()
                    .filter(|metrics| {
                        truncate_width.is_none()
                            && metrics.line_start <= text.len()
                            && (metrics.line_start == 0
                                || text.as_bytes().get(metrics.line_start - 1) == Some(&b'\n'))
                            && metrics.font_size > Pixels::ZERO
                            && f32::from(metrics.font_size).is_finite()
                            && metrics.line_height > Pixels::ZERO
                            && f32::from(metrics.line_height).is_finite()
                    })
                    .map(|metrics| (metrics.line_start, *metrics))
                    .collect();
                let line_sizes = typography
                    .iter()
                    .map(|(start, metrics)| (*start, metrics.font_size))
                    .collect();
                let shaping_text = if inline_boxes.is_empty() {
                    text.clone()
                } else {
                    let mut shaping = text.to_string();
                    for object in &inline_boxes {
                        if let Some(source) = shaping.get(object.range.clone()) {
                            let source = source.replace('\n', " ");
                            shaping.replace_range(object.range.clone(), &source);
                        }
                    }
                    shaping.into()
                };
                let Some(mut lines) = window
                    .text_system()
                    .shape_text_with_line_sizes(
                        shaping_text,
                        font_size,
                        &runs,
                        wrap_width,            // Wrap if we know the width.
                        text_style.line_clamp, // Limit the number of lines if line_clamp is set.
                        &line_sizes,
                    )
                    .log_err()
                else {
                    element_state.0.borrow_mut().replace(TextLayoutInner {
                        text: "".into(),
                        lines: Default::default(),
                        len: 0,
                        line_height,
                        wrap_width,
                        size: Some(Size::default()),
                        bounds: None,
                        line_top_spacing: Vec::new(),
                        line_indents: Vec::new(),
                        line_heights: Vec::new(),
                    });
                    return Size::default();
                };

                let mut size: Size<Pixels> = Size::default();
                let spacing: std::collections::HashMap<_, _> =
                    line_top_spacing.iter().copied().collect();
                let mut line_start = 0;
                let mut resolved_spacing = Vec::new();
                let indents: std::collections::HashMap<_, _> = line_indents
                    .iter()
                    .map(|indent| (indent.line_start, indent))
                    .collect();
                let mut resolved_indents = Vec::new();
                let heights: std::collections::HashMap<_, _> =
                    empty_line_heights.iter().copied().collect();
                let mut resolved_heights = Vec::new();
                let mut remaining_wraps = text_style.line_clamp.map(|max| max.saturating_sub(1));
                for line in &mut lines {
                    if !inline_boxes.is_empty() {
                        line.text = text[line_start..line_start + line.len()].to_owned().into();
                    }
                    let objects: Vec<_> = inline_boxes
                        .iter()
                        .filter(|object| {
                            truncate_width.is_none()
                                && object.range.start >= line_start
                                && object.range.end <= line_start + line.len()
                        })
                        .map(|object| {
                            let mut object = object.clone();
                            object.range =
                                object.range.start - line_start..object.range.end - line_start;
                            object
                        })
                        .collect();
                    if !objects.is_empty() {
                        *line = std::mem::take(line).with_inline_boxes(&objects, remaining_wraps);
                    }
                    let indent = indents.get(&line_start).filter(|indent| {
                        indent.width >= Pixels::ZERO
                            && f32::from(indent.width).is_finite()
                            && line.text.is_char_boundary(indent.prefix_len)
                            && !line.runs().iter().flat_map(|run| &run.glyphs).any(|glyph| {
                                glyph.index < indent.prefix_len
                                    && glyph.cluster_end > indent.prefix_len
                            })
                    });
                    let inset = indent.map_or(Pixels::ZERO, |indent| indent.width);
                    if let Some(indent) = indent {
                        *line = std::mem::take(line).with_collapsed_prefix(
                            indent.prefix_len,
                            inset,
                            remaining_wraps,
                        );
                    }
                    if let Some(remaining) = &mut remaining_wraps {
                        *remaining = remaining.saturating_sub(line.wrap_boundaries.len());
                    }
                    if !indents.is_empty() {
                        resolved_indents.push(inset);
                    }
                    let top = spacing
                        .get(&line_start)
                        .copied()
                        .unwrap_or_default()
                        .max(Pixels::ZERO);
                    if !spacing.is_empty() {
                        resolved_spacing.push(top);
                    }
                    size.height += top;
                    let mut row_height = heights
                        .get(&line_start)
                        .copied()
                        .filter(|height| {
                            line.len() == 0
                                && *height > Pixels::ZERO
                                && f32::from(*height).is_finite()
                        })
                        .unwrap_or_else(|| {
                            typography
                                .get(&line_start)
                                .map_or(line_height, |metrics| metrics.line_height)
                        });
                    if !objects.is_empty() {
                        row_height = row_height.max(line.ascent() + line.descent());
                    }
                    if !heights.is_empty() || !typography.is_empty() || !inline_boxes.is_empty() {
                        resolved_heights.push(row_height);
                    }
                    line_start += line.len() + 1;
                    let line_size = line.size(row_height);
                    size.height += line_size.height;
                    size.width = size.width.max(line_size.width + inset).ceil();
                }

                element_state.0.borrow_mut().replace(TextLayoutInner {
                    text,
                    lines,
                    len,
                    line_height,
                    wrap_width,
                    size: Some(size),
                    bounds: None,
                    line_top_spacing: resolved_spacing,
                    line_indents: resolved_indents,
                    line_heights: resolved_heights,
                });

                size
            }
        })
    }

    fn prepaint(&self, bounds: Bounds<Pixels>, text: &str, _window: &mut Window) {
        let mut element_state = self.0.borrow_mut();
        let element_state = element_state
            .as_mut()
            .with_context(|| format!("measurement has not been performed on {text}"))
            .unwrap();
        element_state.bounds = Some(bounds);
        #[cfg(any(feature = "inspector", debug_assertions))]
        _window.inspect_text(text, bounds, element_state.line_height);
    }

    fn paint(&self, text: &str, window: &mut Window, cx: &mut App) {
        self.paint_with_selection(text, None, window, cx);
    }

    fn paint_with_selection(
        &self,
        text: &str,
        selection: Option<(Range<usize>, crate::Hsla)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let quads = selection.map(|(range, color)| {
            selection::selection_quads(self, range, window.text_style().text_align, color)
        });
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .with_context(|| format!("measurement has not been performed on {text}"))
            .unwrap();
        let bounds = element_state
            .bounds
            .with_context(|| format!("prepaint has not been performed on {text}"))
            .unwrap();

        let mut line_origin = bounds.origin;
        let text_style = window.text_style();
        for (index, line) in element_state.lines.iter().enumerate() {
            let line_height = element_state.height_for_line(index);
            let indent = element_state
                .line_indents
                .get(index)
                .copied()
                .unwrap_or_default();
            line_origin.x = bounds.left() + indent;
            let mut line_bounds = bounds;
            line_bounds.origin.x += indent;
            line_bounds.size.width = (line_bounds.size.width - indent).max(Pixels::ZERO);
            line_origin.y += element_state
                .line_top_spacing
                .get(index)
                .copied()
                .unwrap_or_default();
            line.paint_background(
                line_origin,
                line_height,
                text_style.text_align,
                Some(line_bounds),
                window,
                cx,
            )
            .log_err();
            if quads.is_none() {
                line.paint(
                    line_origin,
                    line_height,
                    text_style.text_align,
                    Some(line_bounds),
                    window,
                    cx,
                )
                .log_err();
            }
            line_origin.y += line.size(line_height).height;
        }
        if let Some(quads) = quads {
            for quad in quads {
                window.paint_quad(quad);
            }
            let mut line_origin = bounds.origin;
            for (index, line) in element_state.lines.iter().enumerate() {
                let line_height = element_state.height_for_line(index);
                let indent = element_state
                    .line_indents
                    .get(index)
                    .copied()
                    .unwrap_or_default();
                line_origin.x = bounds.left() + indent;
                let mut line_bounds = bounds;
                line_bounds.origin.x += indent;
                line_bounds.size.width = (line_bounds.size.width - indent).max(Pixels::ZERO);
                line_origin.y += element_state
                    .line_top_spacing
                    .get(index)
                    .copied()
                    .unwrap_or_default();
                line.paint(
                    line_origin,
                    line_height,
                    text_style.text_align,
                    Some(line_bounds),
                    window,
                    cx,
                )
                .log_err();
                line_origin.y += line.size(line_height).height;
            }
        }
    }

    /// Get the byte index into the input of the pixel position.
    pub fn index_for_position(&self, mut position: Point<Pixels>) -> Result<usize, usize> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");

        if position.y < bounds.top() {
            return Err(0);
        }

        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;
        for (index, line) in element_state.lines.iter().enumerate() {
            let line_height = element_state.height_for_line(index);
            line_origin.x = bounds.left()
                + element_state
                    .line_indents
                    .get(index)
                    .copied()
                    .unwrap_or_default();
            line_origin.y += element_state
                .line_top_spacing
                .get(index)
                .copied()
                .unwrap_or_default();
            if position.y < line_origin.y {
                return Err(line_start_ix);
            }
            let line_bottom = line_origin.y + line.size(line_height).height;
            if position.y > line_bottom {
                line_origin.y = line_bottom;
                line_start_ix += line.len() + 1;
            } else {
                let position_within_line = position - line_origin;
                match line.index_for_position(position_within_line, line_height) {
                    Ok(index_within_line) => return Ok(line_start_ix + index_within_line),
                    Err(index_within_line) => return Err(line_start_ix + index_within_line),
                }
            }
        }

        Err(line_start_ix.saturating_sub(1))
    }

    /// Get the pixel position for the given byte index.
    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");

        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;

        for (line_index, line) in element_state.lines.iter().enumerate() {
            let line_height = element_state.height_for_line(line_index);
            line_origin.x = bounds.left()
                + element_state
                    .line_indents
                    .get(line_index)
                    .copied()
                    .unwrap_or_default();
            line_origin.y += element_state
                .line_top_spacing
                .get(line_index)
                .copied()
                .unwrap_or_default();
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_origin.y += line.size(line_height).height;
                line_start_ix = line_end_ix + 1;
                continue;
            } else {
                let ix_within_line = index - line_start_ix;
                return Some(line_origin + line.position_for_index(ix_within_line, line_height)?);
            }
        }

        None
    }

    /// Retrieve the layout for the line containing the given byte index.
    pub fn line_layout_for_index(&self, index: usize) -> Option<Arc<WrappedLineLayout>> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let mut line_start_ix = 0;

        for line in &element_state.lines {
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_start_ix = line_end_ix + 1;
                continue;
            } else {
                return Some(line.layout.clone());
            }
        }

        None
    }

    /// Retrieve all line layouts in source order.
    pub fn line_layouts(&self) -> SmallVec<[Arc<WrappedLineLayout>; 1]> {
        self.0
            .borrow()
            .as_ref()
            .expect("measurement has not been performed")
            .lines
            .iter()
            .map(|line| line.layout.clone())
            .collect()
    }

    /// Position on the text baseline for a UTF-8 byte offset, after prepaint.
    pub fn baseline_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let mut position = self.position_for_index(index)?;
        let state = self.0.borrow();
        let state = state.as_ref()?;
        let mut start = 0;
        for (line_index, line) in state.lines.iter().enumerate() {
            if index <= start + line.len() {
                let height = state.height_for_line(line_index);
                position.y += (height - line.ascent() - line.descent()) / 2. + line.ascent();
                return Some(position);
            }
            start += line.len() + 1;
        }
        None
    }

    /// The bounds of this layout.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().as_ref().unwrap().bounds.unwrap()
    }

    /// The line height for this layout.
    pub fn line_height(&self) -> Pixels {
        self.0.borrow().as_ref().unwrap().line_height
    }

    /// The UTF-8 length of the underlying text.
    pub fn len(&self) -> usize {
        self.0.borrow().as_ref().unwrap().len
    }

    /// The text for this layout.
    pub fn text(&self) -> String {
        self.0
            .borrow()
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .map(|s| &s.text)
            .join("\n")
    }

    /// The text for this layout (with soft-wraps as newlines)
    pub fn wrapped_text(&self) -> String {
        let mut accumulator = String::new();

        for wrapped in self.0.borrow().as_ref().unwrap().lines.iter() {
            if wrapped.wrap_boundaries().is_empty() {
                accumulator.push_str(&wrapped.text);
                accumulator.push('\n');
                continue;
            }
            for row in 0..=wrapped.wrap_boundaries().len() {
                for range in wrapped.row_text_ranges(row) {
                    accumulator.push_str(&wrapped.text[range]);
                }
                accumulator.push('\n');
            }
        }
        // Remove trailing newline
        accumulator.pop();
        accumulator
    }
}

/// A text element that can be interacted with.
pub struct InteractiveText {
    element_id: ElementId,
    text: StyledText,
    click_listener:
        Option<Box<dyn Fn(&[Range<usize>], InteractiveTextClickEvent, &mut Window, &mut App)>>,
    hover_listener: Option<Box<dyn Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut App)>>,
    tooltip_builder: Option<Rc<dyn Fn(usize, &mut Window, &mut App) -> Option<AnyView>>>,
    tooltip_id: Option<TooltipId>,
    clickable_ranges: Vec<Range<usize>>,
    selection_color: Option<crate::Hsla>,
    preserve_selection_on_update: bool,
    selection_revision: u64,
}

struct InteractiveTextClickEvent {
    mouse_down_index: usize,
    mouse_up_index: usize,
}

#[doc(hidden)]
#[derive(Default)]
pub struct InteractiveTextState {
    mouse_down_index: Rc<Cell<Option<usize>>>,
    hovered_index: Rc<Cell<Option<usize>>>,
    active_tooltip: Rc<RefCell<Option<ActiveTooltip>>>,
    tooltip_geometry: Option<crate::ElementBounds>,
    selection: Option<Rc<RefCell<TextSelection>>>,
}

/// InteractiveTest is a wrapper around StyledText that adds mouse interactions.
impl InteractiveText {
    /// Creates a new InteractiveText from the given text.
    pub fn new(id: impl Into<ElementId>, text: StyledText) -> Self {
        Self {
            element_id: id.into(),
            text,
            click_listener: None,
            hover_listener: None,
            tooltip_builder: None,
            tooltip_id: None,
            clickable_ranges: Vec::new(),
            selection_color: None,
            preserve_selection_on_update: false,
            selection_revision: 0,
        }
    }

    /// Enable read-only selection within this text block using the given highlight color.
    /// Drag to select, long-press to select a word, or Shift-click to extend.
    /// Use Ctrl/Cmd+A or Ctrl/Cmd+C while focused to select all or copy plain text. Styles are inherited from
    /// the parent and retained while selecting. Selection resets when the displayed
    /// text changes. Ellipsized text copies its displayed representation; links are
    /// only activated when layout preserves the original text offsets.
    pub fn selectable(mut self, selection_color: impl Into<crate::Hsla>) -> Self {
        self.selection_color = Some(selection_color.into());
        self
    }

    /// Retain the selection when text through its final selected byte is unchanged.
    /// Updates affecting that prefix reset the selection. Disabled by default.
    pub fn preserve_selection_on_update(mut self, preserve: bool) -> Self {
        self.preserve_selection_on_update = preserve;
        self
    }

    /// Change this value to reset selection even when the displayed text is unchanged.
    pub fn selection_revision(mut self, revision: u64) -> Self {
        self.selection_revision = revision;
        self
    }

    /// on_click is called when the user clicks on one of the given ranges, passing the index of
    /// the clicked range.
    pub fn on_click(
        mut self,
        ranges: Vec<Range<usize>>,
        listener: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click_listener = Some(Box::new(move |ranges, event, window, cx| {
            for (range_ix, range) in ranges.iter().enumerate() {
                if range.contains(&event.mouse_down_index) && range.contains(&event.mouse_up_index)
                {
                    listener(range_ix, window, cx);
                }
            }
        }));
        self.clickable_ranges = ranges;
        self
    }

    /// on_hover is called when the mouse moves over a character within the text, passing the
    /// index of the hovered character, or None if the mouse leaves the text.
    pub fn on_hover(
        mut self,
        listener: impl Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.hover_listener = Some(Box::new(listener));
        self
    }

    /// tooltip lets you specify a tooltip for a given character index in the string.
    pub fn tooltip(
        mut self,
        builder: impl Fn(usize, &mut Window, &mut App) -> Option<AnyView> + 'static,
    ) -> Self {
        self.tooltip_builder = Some(Rc::new(builder));
        self
    }
}

impl Element for InteractiveText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::Label)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.text.text.to_string());
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        window.with_optional_element_state::<InteractiveTextState, _>(
            global_id,
            |interactive_state, window| {
                let mut interactive_state = interactive_state
                    .map(|interactive_state| interactive_state.unwrap_or_default());

                if let Some(interactive_state) = interactive_state.as_mut() {
                    if self.selection_color.is_some() {
                        let selection = interactive_state
                            .selection
                            .get_or_insert_with(|| Rc::new(RefCell::new(TextSelection::new(cx))));
                        let displayed_text =
                            self.text.layout.0.borrow().as_ref().unwrap().text.clone();
                        if selection.borrow_mut().sync_text(
                            displayed_text,
                            self.preserve_selection_on_update,
                            self.selection_revision,
                        ) {
                            interactive_state.mouse_down_index.set(None);
                        }
                        window.set_focus_handle(&selection.borrow().focus, cx);
                    } else {
                        interactive_state.selection = None;
                    }
                    if self.tooltip_builder.is_some() {
                        self.tooltip_id =
                            set_tooltip_on_window(&interactive_state.active_tooltip, window);
                    } else {
                        // If there is no longer a tooltip builder, remove the active tooltip.
                        interactive_state.active_tooltip.take();
                    }
                }

                self.text
                    .prepaint(None, inspector_id, bounds, state, window, cx);
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                if self.tooltip_builder.is_some()
                    && let Some(state) = interactive_state.as_mut()
                {
                    let geometry = state.tooltip_geometry.get_or_insert_with(Default::default);
                    window.track_element_hitbox(geometry, &hitbox);
                }
                (hitbox, interactive_state)
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let current_view = window.current_view();
        let text_layout = self.text.layout().clone();
        window.with_element_state::<InteractiveTextState, _>(
            global_id.unwrap(),
            |interactive_state, window| {
                let mut interactive_state = interactive_state.unwrap_or_default();
                if let Some(selection) = &interactive_state.selection {
                    let ranges = if text_layout.0.borrow().as_ref().unwrap().text == self.text.text
                    {
                        mem::take(&mut self.clickable_ranges)
                    } else {
                        Vec::new()
                    };
                    selection::register_handlers(
                        selection.clone(),
                        text_layout.clone(),
                        hitbox.clone(),
                        interactive_state.mouse_down_index.clone(),
                        self.click_listener.take(),
                        ranges,
                        window,
                        cx,
                    );
                } else if let Some(click_listener) = self.click_listener.take() {
                    let mouse_position = window.mouse_position();
                    if let Ok(ix) = text_layout.index_for_position(mouse_position)
                        && self
                            .clickable_ranges
                            .iter()
                            .any(|range| range.contains(&ix))
                    {
                        window.set_cursor_style(crate::CursorStyle::PointingHand, hitbox)
                    }

                    let text_layout = text_layout.clone();
                    let mouse_down = interactive_state.mouse_down_index.clone();
                    if let Some(mouse_down_index) = mouse_down.get() {
                        let hitbox = hitbox.clone();
                        let clickable_ranges = mem::take(&mut self.clickable_ranges);
                        window.on_mouse_event(
                            move |event: &MouseUpEvent, phase, window: &mut Window, cx| {
                                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                                    if let Ok(mouse_up_index) =
                                        text_layout.index_for_position(event.position)
                                    {
                                        click_listener(
                                            &clickable_ranges,
                                            InteractiveTextClickEvent {
                                                mouse_down_index,
                                                mouse_up_index,
                                            },
                                            window,
                                            cx,
                                        )
                                    }

                                    mouse_down.take();
                                    window.refresh();
                                }
                            },
                        );
                    } else {
                        let hitbox = hitbox.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                            if phase == DispatchPhase::Bubble
                                && hitbox.is_hovered(window)
                                && let Ok(mouse_down_index) =
                                    text_layout.index_for_position(event.position)
                            {
                                mouse_down.set(Some(mouse_down_index));
                                window.refresh();
                            }
                        });
                    }
                }

                window.on_mouse_event({
                    let mut hover_listener = self.hover_listener.take();
                    let hitbox = hitbox.clone();
                    let text_layout = text_layout.clone();
                    let hovered_index = interactive_state.hovered_index.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                            let current = hovered_index.get();
                            let updated = text_layout.index_for_position(event.position).ok();
                            if current != updated {
                                hovered_index.set(updated);
                                if let Some(hover_listener) = hover_listener.as_ref() {
                                    hover_listener(updated, event.clone(), window, cx);
                                }
                                cx.notify(current_view);
                            }
                        }
                    }
                });

                if let Some(tooltip_builder) = self.tooltip_builder.clone() {
                    let active_tooltip = interactive_state.active_tooltip.clone();
                    let build_tooltip = Rc::new({
                        let tooltip_is_hoverable = false;
                        let text_layout = text_layout.clone();
                        let geometry = interactive_state.tooltip_geometry.clone().unwrap();
                        move |window: &mut Window, cx: &mut App| {
                            let position =
                                geometry.hit_position(window.raw_mouse_position(), window)?;
                            text_layout
                                .index_for_position(position)
                                .ok()
                                .and_then(|position| tooltip_builder(position, window, cx))
                                .map(|view| (view, tooltip_is_hoverable))
                        }
                    });

                    let check_is_hovered_during_prepaint = Rc::new({
                        let geometry = interactive_state.tooltip_geometry.clone().unwrap();
                        let text_layout = text_layout.clone();
                        let pending_mouse_down = interactive_state.mouse_down_index.clone();
                        move |window: &Window| {
                            geometry
                                .unoccluded_hit_position(window.raw_mouse_position(), window)
                                .is_some_and(|position| {
                                    text_layout.index_for_position(position).is_ok()
                                })
                                && pending_mouse_down.get().is_none()
                        }
                    });

                    let check_is_hovered = Rc::new({
                        let hitbox = hitbox.clone();
                        let text_layout = text_layout.clone();
                        let pending_mouse_down = interactive_state.mouse_down_index.clone();
                        move |window: &Window| {
                            text_layout
                                .index_for_position(window.mouse_position())
                                .is_ok()
                                && hitbox.is_hovered(window)
                                && pending_mouse_down.get().is_none()
                        }
                    });

                    register_tooltip_mouse_handlers(
                        &active_tooltip,
                        self.tooltip_id,
                        build_tooltip,
                        check_is_hovered,
                        check_is_hovered_during_prepaint,
                        None,
                        window,
                    );
                }

                let selected = interactive_state.selection.as_ref().and_then(|state| {
                    let state = state.borrow();
                    self.selection_color.map(|color| (state.range(), color))
                });
                self.text
                    .layout
                    .paint_with_selection(&self.text.text, selected, window, cx);

                ((), interactive_state)
            },
        );
    }
}

impl IntoElement for InteractiveText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_element_for() {
        use crate::{ParentElement as _, SharedString, div};
        use std::borrow::Cow;

        let _ = div().child("static str");
        let _ = div().child("String".to_string());
        let _ = div().child(Cow::Borrowed("Cow"));
        let _ = div().child(SharedString::from("SharedString"));
    }

    #[test]
    fn text_macro_id() {
        // one call to `text!` = one id
        fn make_text_stable_id(happy: bool) -> Text {
            text!(if happy { "happy" } else { "sad" })
        }

        // two calls to `text!` = two ids
        fn make_text_unstable_id(happy: bool) -> Text {
            if happy { text!("happy") } else { text!("sad") }
        }

        assert_eq!(make_text_stable_id(false).id, make_text_stable_id(true).id);
        assert_ne!(
            make_text_unstable_id(false).id,
            make_text_unstable_id(true).id
        );
    }
}
