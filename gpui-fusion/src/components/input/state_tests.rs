use std::{cell::RefCell, rc::Rc};

use gpui::{
    Context, Entity, EntityInputHandler, IntoElement, Render, SharedString, Subscription,
    TestAppContext, VisualTestContext, Window, px, size,
};

use super::*;
use crate::components::input::Input;

struct BidiText;

impl gpui::PlatformTextSystem for BidiText {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
        gpui::NoopTextSystem.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        gpui::NoopTextSystem.all_font_names()
    }
    fn font_id(&self, font: &gpui::Font) -> gpui::Result<gpui::FontId> {
        gpui::NoopTextSystem.font_id(font)
    }
    fn font_metrics(&self, id: gpui::FontId) -> gpui::FontMetrics {
        gpui::NoopTextSystem.font_metrics(id)
    }
    fn typographic_bounds(
        &self,
        font: gpui::FontId,
        glyph: gpui::GlyphId,
    ) -> gpui::Result<Bounds<f32>> {
        gpui::NoopTextSystem.typographic_bounds(font, glyph)
    }
    fn advance(&self, font: gpui::FontId, glyph: gpui::GlyphId) -> gpui::Result<gpui::Size<f32>> {
        gpui::NoopTextSystem.advance(font, glyph)
    }
    fn glyph_for_char(&self, font: gpui::FontId, ch: char) -> Option<gpui::GlyphId> {
        gpui::NoopTextSystem.glyph_for_char(font, ch)
    }
    fn glyph_raster_bounds(
        &self,
        params: &gpui::RenderGlyphParams,
    ) -> gpui::Result<Bounds<gpui::DevicePixels>> {
        gpui::NoopTextSystem.glyph_raster_bounds(params)
    }
    fn rasterize_glyph(
        &self,
        params: &gpui::RenderGlyphParams,
        bounds: Bounds<gpui::DevicePixels>,
    ) -> gpui::Result<(gpui::Size<gpui::DevicePixels>, Vec<u8>)> {
        gpui::NoopTextSystem.rasterize_glyph(params, bounds)
    }
    fn recommended_rendering_mode(&self, _: gpui::FontId, _: Pixels) -> gpui::TextRenderingMode {
        gpui::TextRenderingMode::Grayscale
    }
    fn layout_line(&self, text: &str, font_size: Pixels, _: &[gpui::FontRun]) -> gpui::LineLayout {
        let glyphs: &[(usize, usize, bool)] = match text {
            "aאב12" => &[
                (0, 1, false),
                (5, 6, false),
                (6, 7, false),
                (3, 5, true),
                (1, 3, true),
            ],
            "אב" => &[(2, 4, true), (0, 2, true)],
            "ב" => &[(0, 2, true)],
            "א\u{5b0}ב" => &[(4, 6, true), (0, 4, true)],
            "אב😀" => &[(4, 8, true), (2, 4, true), (0, 2, true)],
            "" => &[],
            _ => panic!("unexpected test text: {text}"),
        };
        gpui::LineLayout {
            font_size,
            width: px(glyphs.len() as f32 * 10.),
            ascent: px(12.),
            descent: px(4.),
            len: text.len(),
            runs: vec![gpui::ShapedRun {
                font_id: gpui::FontId(0),
                glyphs: glyphs
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(visual, (index, cluster_end, is_rtl))| gpui::ShapedGlyph {
                        id: gpui::GlyphId(0),
                        position: point(px(visual as f32 * 10.), px(0.)),
                        index,
                        cluster_end,
                        advance: px(10.),
                        is_rtl,
                        is_emoji: false,
                    })
                    .collect(),
            }],
        }
    }
}

fn bidi_text_layout(text: &str, wrap_width: Option<Pixels>) -> TextLayout {
    use std::sync::Arc;
    let shaper = gpui::WindowTextSystem::new(Arc::new(gpui::TextSystem::new(Arc::new(BidiText))));
    let lines = shaper
        .shape_text(
            text.to_owned().into(),
            px(16.),
            &[gpui::TextRun {
                len: text.len(),
                ..Default::default()
            }],
            wrap_width,
            None,
        )
        .unwrap()
        .into_vec();
    let starts = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    TextLayout::new(lines, starts, px(20.))
}

#[gpui::test]
fn bidi_deletion_removes_only_the_visual_neighbor(cx: &mut TestAppContext) {
    let window = open_input(cx, InputState::new);
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                for (text, index, affinity, right, expected) in [
                    ("אב", 0, CaretAffinity::Downstream, false, "ב"),
                    ("אב", 4, CaretAffinity::Upstream, true, "א"),
                    ("aאב12", 1, CaretAffinity::Downstream, false, "aב12"),
                    ("aאב12", 1, CaretAffinity::Upstream, true, "aאב2"),
                    ("aאב12", 7, CaretAffinity::Upstream, false, "aאב1"),
                    ("aאב12", 5, CaretAffinity::Upstream, true, "aא12"),
                    ("aאב12", 1, CaretAffinity::Upstream, false, "אב12"),
                    ("א\u{5b0}ב", 0, CaretAffinity::Downstream, false, "ב"),
                    ("אב😀", 4, CaretAffinity::Upstream, false, "אב"),
                ] {
                    input.set_value(text, cx);
                    input.last_layout = Some(bidi_text_layout(text, None));
                    input.move_to(index, cx);
                    input.caret_affinity = affinity;
                    if right {
                        input.delete(&Delete, window, cx);
                    } else {
                        input.backspace(&Backspace, window, cx);
                    }
                    assert_eq!(input.content.as_ref(), expected, "{text}, {index}, {right}");
                    assert!(input.selected_range.is_empty());
                    assert!(input.content.is_char_boundary(input.cursor_offset()));
                    if text == "aאב12"
                        && index == 1
                        && affinity == CaretAffinity::Upstream
                        && !right
                    {
                        assert_eq!(input.cursor_offset(), 4);
                        assert_eq!(input.caret_affinity, CaretAffinity::Downstream);
                    }
                }
                input.set_value("aאב12", cx);
                input.last_layout = Some(bidi_text_layout("aאב12", None));
                input.move_to(1, cx);
                input.select_to(5, cx);
                input.backspace(&Backspace, window, cx);
                assert_eq!(input.content.as_ref(), "a12");
                for (right, index) in [(false, 0), (true, 1)] {
                    input.set_value("aאב12", cx);
                    input.last_layout = Some(bidi_text_layout("aאב12", None));
                    input.move_to(index, cx);
                    input.delete_horizontal(right, window, cx);
                    assert_eq!(input.content.as_ref(), "aאב12");
                }
                input.disabled = true;
                input.move_to(1, cx);
                input.backspace(&Backspace, window, cx);
                assert_eq!(input.content.as_ref(), "aאב12");
            });
        })
        .unwrap();
}

#[test]
fn bidi_repeated_backspace_refreshes_changed_text_before_next_frame() {
    use std::sync::Arc;
    let shaper = gpui::WindowTextSystem::new(Arc::new(gpui::TextSystem::new(Arc::new(BidiText))));
    let mut content: SharedString = "אב".into();
    let mut layout = bidi_text_layout(&content, None);
    layout.shaping_run = Some(gpui::TextRun {
        len: content.len(),
        ..Default::default()
    });
    let mut caret = TextCaret::default();
    for expected in ["ב", ""] {
        layout.refresh_for_edit(&content, &shaper);
        let (range, next) = layout.bidi_deletion_target(&content, caret, false).unwrap();
        let mut text = content.to_string();
        text.replace_range(range, "");
        content = text.into();
        caret = next;
        assert_eq!(content.as_ref(), expected);
        assert_eq!(caret.index, 0);
    }
}

#[test]
fn bidi_continuous_backspace_with_real_shaping() {
    use gpui::PlatformTextSystem;
    use std::sync::Arc;
    let system = gpui_wgpu::CosmicTextSystem::new_without_system_fonts("Lilex");
    system
        .add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../../../assets/fonts/lilex/Lilex-Regular.ttf"
        ))])
        .unwrap();
    let shaper = gpui::WindowTextSystem::new(Arc::new(gpui::TextSystem::new(Arc::new(system))));
    let mut content: SharedString = "English אבג 123 العربية 中文".into();
    let run = gpui::TextRun {
        len: content.len(),
        font: gpui::font("Lilex"),
        ..Default::default()
    };
    let lines = shaper
        .shape_text(
            content.clone(),
            px(20.),
            std::slice::from_ref(&run),
            None,
            None,
        )
        .unwrap()
        .into_vec();
    let mut layout = TextLayout::new(lines, vec![0], px(24.));
    layout.shaping_run = Some(run);
    let mut caret: TextCaret = content.find("中文").unwrap().into();
    let mut deleted = String::new();
    let mut reordered = false;
    for _ in 0..30 {
        layout.refresh_for_edit(&content, &shaper);
        if content.contains("العربية") {
            let left = |word: &str| {
                let start = content.find(word).unwrap();
                layout.lines[0]
                    .selection_bounds(start..start + word.len(), layout.line_height)
                    .iter()
                    .map(|bounds| bounds.left())
                    .min()
                    .unwrap()
            };
            if content.contains(['א', 'ב', 'ג']) {
                assert!(left("العربية") < left("123"));
            } else {
                assert!(left("123") < left("العربية"));
                reordered = true;
            }
        }
        let (range, next) = layout
            .bidi_deletion_target(&content, caret, false)
            .unwrap_or_else(|| {
                let prev = content
                    .grapheme_indices(true)
                    .rev()
                    .find_map(|(i, _)| (i < caret.index).then_some(i))
                    .unwrap_or(0);
                (prev..caret.index, prev.into())
            });
        if range.is_empty() {
            break;
        }
        assert_eq!(content[range.clone()].graphemes(true).count(), 1);
        deleted.push_str(&content[range.clone()]);
        let mut text = content.to_string();
        text.replace_range(range, "");
        content = text.into();
        caret = next;
        assert_eq!(&content[caret.index..], "中文");
    }
    assert!(reordered);
    assert_eq!(deleted, " אבגالعربية 321  hsilgnE");
    assert_eq!(content.as_ref(), "中文");
}

#[test]
fn bidi_arrows_follow_visual_rows_and_stop_at_edges() {
    for (text, wrap_width) in [
        ("אב", None),
        ("aאב12", None),
        ("aאב12", Some(px(30.))),
        ("aאב12\naאב12", None),
    ] {
        let layout = bidi_text_layout(text, wrap_width);
        let mut caret = layout.caret_for_position(Point::default());
        let mut positions = vec![layout.position_for_caret(caret.index, caret.affinity)];
        for _ in 0..30 {
            let next = layout
                .bidi_horizontal_target(text, caret, true, None)
                .unwrap();
            if next == caret {
                break;
            }
            let position = layout.position_for_caret(next.index, next.affinity);
            let previous = positions.last().unwrap();
            assert!((position.y, position.x) > (previous.y, previous.x));
            positions.push(position);
            caret = next;
        }
        assert!(positions.len() >= if text == "אב" { 3 } else { 6 });
        if wrap_width.is_some() || text.contains('\n') {
            assert!(positions.last().unwrap().y > px(0.));
        }
        assert_eq!(
            layout.bidi_horizontal_target(text, caret, true, None),
            Some(caret)
        );
        for expected in positions.iter().rev().skip(1) {
            caret = layout
                .bidi_horizontal_target(text, caret, false, None)
                .unwrap();
            assert_eq!(
                layout.position_for_caret(caret.index, caret.affinity),
                *expected
            );
        }
        assert_eq!(
            layout.bidi_horizontal_target(text, caret, false, None),
            Some(caret)
        );
        assert!(
            layout
                .bidi_horizontal_target("changed", caret, true, None)
                .is_none()
        );
        assert!(
            layout
                .bidi_horizontal_target(&format!("{text}z"), caret, true, None)
                .is_none()
        );
    }
}

#[gpui::test]
fn horizontal_arrows_preserve_ltr_grapheme_steps(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("a\u{301}😀中文"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.move_to(0, cx);
                for end in [3, 7, 10, 13, 13] {
                    input.right(&Right, window, cx);
                    assert_eq!(input.selected_range, end..end);
                }
                for start in [10, 7, 3, 0, 0] {
                    input.select_left(&SelectLeft, window, cx);
                    assert_eq!(input.selected_range, start..13);
                }
                for start in [3, 7, 10, 13] {
                    input.select_right(&SelectRight, window, cx);
                    assert_eq!(input.selected_range, start..13);
                }
            });
        })
        .unwrap();
}

#[gpui::test]
fn bidi_keyboard_selection_preserves_anchor_and_collapses_visually(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("aאב12"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.last_layout = Some(bidi_text_layout("aאב12", None));
                let caret_x = |input: &InputState| {
                    input
                        .last_layout
                        .as_ref()
                        .unwrap()
                        .position_for_caret(input.cursor_offset(), input.caret_affinity)
                        .x
                };
                for (start, affinity, xs) in [
                    (1, CaretAffinity::Downstream, [40., 30., 40., 50.]),
                    (1, CaretAffinity::Upstream, [0., 0., 10., 20.]),
                ] {
                    input.move_to(start, cx);
                    input.caret_affinity = affinity;
                    input.select_left(&SelectLeft, window, cx);
                    assert_eq!(caret_x(input), px(xs[0]));
                    input.select_left(&SelectLeft, window, cx);
                    assert_eq!(caret_x(input), px(xs[1]));
                    input.select_right(&SelectRight, window, cx);
                    assert_eq!(caret_x(input), px(xs[2]));
                    input.select_right(&SelectRight, window, cx);
                    assert_eq!(caret_x(input), px(xs[3]));
                }
                // Both visual representations of the same logical anchor must
                // survive extending and retracting through a direction boundary.
                for affinity in [CaretAffinity::Upstream, CaretAffinity::Downstream] {
                    input.move_to(1, cx);
                    input.caret_affinity = affinity;
                    let right = affinity == CaretAffinity::Upstream;
                    input.move_horizontal(right, true, cx);
                    input.move_horizontal(!right, true, cx);
                    assert_eq!(input.selected_range, 1..1);
                    assert_eq!(input.caret_affinity, affinity);
                }
                for right in [false, true] {
                    input.move_to(1, cx);
                    input.select_to(5, cx);
                    input.move_horizontal(right, false, cx);
                    assert_eq!(caret_x(input), px(if right { 50. } else { 30. }));
                    assert!(input.selected_range.is_empty());
                }
                input.move_to(1, cx);
                input.left(&Left, window, cx);
                assert_eq!(caret_x(input), px(40.));
                input.right(&Right, window, cx);
                assert_eq!(caret_x(input), px(50.));
                input.disabled = true;
                input.left(&Left, window, cx);
                assert_eq!(caret_x(input), px(50.));
            });
        })
        .unwrap();
}

#[gpui::test]
fn bidi_mouse_caret_selection_and_ime_bounds_agree(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("aאב12"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.last_layout = Some(bidi_text_layout("aאב12", None));
                let bounds = Bounds::new(point(px(10.), px(10.)), size(px(100.), px(20.)));
                input.last_bounds = Some(bounds);
                for (x, affinity, caret_x) in [
                    (9., CaretAffinity::Upstream, 10.),
                    (49., CaretAffinity::Downstream, 50.),
                ] {
                    input.on_mouse_down(
                        &MouseDownEvent {
                            position: bounds.origin + point(px(x), px(5.)),
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Default::default(),
                            first_mouse: false,
                        },
                        window,
                        cx,
                    );
                    assert_eq!(input.selected_range, 1..1);
                    assert_eq!(input.caret_affinity, affinity);
                    let rect = input.bounds_for_range(1..1, bounds, window, cx).unwrap();
                    assert_eq!(rect.origin.x, bounds.left() + px(caret_x));
                }
                input.select_to(3, cx);
                let quads = super::super::element::selection_quads(
                    input.last_layout.as_ref().unwrap(),
                    0..3,
                    bounds,
                    gpui::black(),
                );
                assert_eq!(quads.len(), 2);
                assert_eq!(quads[0].bounds.origin.x, bounds.left());
                assert_eq!(quads[1].bounds.origin.x, bounds.left() + px(40.));
                input.on_mouse_down(
                    &MouseDownEvent {
                        position: bounds.origin + point(px(45.), px(5.)),
                        button: MouseButton::Left,
                        click_count: 2,
                        modifiers: Default::default(),
                        first_mouse: false,
                    },
                    window,
                    cx,
                );
                assert_eq!(input.selected_range, 0..7);
                input.stop_selection();
            });
        })
        .unwrap();
}

struct TestInput {
    state: Entity<InputState>,
    rows: Option<usize>,
    changes: Rc<RefCell<Vec<SharedString>>>,
    _subscription: Subscription,
}

#[test]
fn layout_cache_tracks_content_width_font_and_preedit_style() {
    use std::sync::Arc;
    let shaper = gpui::WindowTextSystem::new(Arc::new(gpui::TextSystem::new(Arc::new(
        gpui::NoopTextSystem,
    ))));
    let text: SharedString = (0..96)
        .map(|i| char::from(b'a' + i % 26))
        .collect::<String>()
        .into();
    let run = gpui::TextRun {
        len: text.len(),
        ..Default::default()
    };
    let settings = TextLayoutSettings {
        font_size: px(16.),
        wrap_width: Some(px(120.)),
        line_height: px(20.),
        tab_size: 4,
    };
    let original = TextLayout::shape(text.clone(), vec![run.clone()], settings, &shaper, None);
    let cached = TextLayout::shape(
        text.clone(),
        vec![run.clone()],
        TextLayoutSettings {
            line_height: px(28.),
            ..settings
        },
        &shaper,
        Some(&original),
    );
    assert!(Rc::ptr_eq(&original.lines, &cached.lines));
    assert_eq!(
        cached.position_for_offset(text.len()).y / px(28.),
        original.position_for_offset(text.len()).y / px(20.)
    );
    let narrow = TextLayout::shape(
        text.clone(),
        vec![run.clone()],
        TextLayoutSettings {
            wrap_width: Some(px(60.)),
            ..settings
        },
        &shaper,
        Some(&original),
    );
    assert!(narrow.visual_row_count() > original.visual_row_count());
    let larger = TextLayout::shape(
        text.clone(),
        vec![run.clone()],
        TextLayoutSettings {
            font_size: px(24.),
            ..settings
        },
        &shaper,
        Some(&original),
    );
    assert_eq!(larger.lines[0].font_size(), px(24.));
    let mut marked_run = run.clone();
    marked_run.underline = Some(gpui::UnderlineStyle {
        thickness: px(1.),
        ..Default::default()
    });
    let marked = TextLayout::shape(
        text.clone(),
        vec![marked_run],
        settings,
        &shaper,
        Some(&original),
    );
    assert!(!Rc::ptr_eq(&original.lines, &marked.lines));
    let replacement: SharedString = text.to_uppercase().into();
    let edited = TextLayout::shape(
        replacement.clone(),
        vec![run],
        settings,
        &shaper,
        Some(&original),
    );
    assert_eq!(edited.lines[0].text, replacement);

    let mut text = (0..8)
        .map(|i| char::from(b'a' + i).to_string().repeat(64))
        .collect::<Vec<_>>()
        .join("\n");
    let shape = |text: &str, previous| {
        TextLayout::shape(
            text.to_owned().into(),
            vec![gpui::TextRun {
                len: text.len(),
                ..Default::default()
            }],
            settings,
            &shaper,
            previous,
        )
    };
    let original = shape(&text, None);
    text.insert(original.line_starts[3] + 12, '\n');
    let inserted = shape(&text, Some(&original));
    assert!(Rc::ptr_eq(&inserted.lines[2], &original.lines[2]));
    assert!(Rc::ptr_eq(&inserted.lines[5], &original.lines[4]));
    let fresh = shape(&text, None);
    for (offset, _) in text.char_indices() {
        assert_eq!(
            inserted.position_for_offset(offset),
            fresh.position_for_offset(offset)
        );
    }
    text.replace_range(inserted.line_starts[3]..inserted.line_starts[5], "");
    let deleted = shape(&text, Some(&inserted));
    assert!(Rc::ptr_eq(&deleted.lines[3], &original.lines[4]));
    assert!(deleted.matches_content(&text));
    assert_eq!(deleted.line_starts.len(), 7);
}

#[test]
fn tab_size_updates_cached_layout_and_edit_geometry() {
    use gpui::PlatformTextSystem;
    use std::{borrow::Cow, sync::Arc};
    let system = gpui_wgpu::CosmicTextSystem::new_without_system_fonts("Lilex");
    system
        .add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../../../assets/fonts/lilex/Lilex-Regular.ttf"
        ))])
        .unwrap();
    let shaper = gpui::WindowTextSystem::new(Arc::new(gpui::TextSystem::new(Arc::new(system))));
    let text: SharedString = "r\ts\n\tt".into();
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::font("Lilex"),
        ..Default::default()
    };
    let settings = TextLayoutSettings {
        font_size: px(15.),
        wrap_width: None,
        line_height: px(21.),
        tab_size: 3,
    };
    let first = TextLayout::shape(text.clone(), vec![run.clone()], settings, &shaper, None);
    let cached = TextLayout::shape(
        text.clone(),
        vec![run.clone()],
        settings,
        &shaper,
        Some(&first),
    );
    assert!(Rc::ptr_eq(&first.lines, &cached.lines));
    let mut changed = TextLayout::shape(
        text.clone(),
        vec![run.clone()],
        TextLayoutSettings {
            tab_size: 6,
            ..settings
        },
        &shaper,
        Some(&cached),
    );
    assert_eq!(
        changed.position_for_offset(2).x,
        first.position_for_offset(2).x * 2.
    );
    assert_eq!(
        changed.position_for_offset(5).x,
        changed.position_for_offset(2).x
    );
    assert!(changed.matches_content(&text));
    changed.shaping_run = Some(run);
    let edited: SharedString = "rr\ts\n\tt".into();
    changed.refresh_for_edit(&edited, &shaper);
    assert!(changed.matches_content(&edited));
    assert_eq!(
        changed.position_for_offset(3).x,
        first.position_for_offset(2).x * 2.
    );
}

impl Render for TestInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::style::Styled as _;
        cx.new(|cx| {
            Input::new(&self.state, window, cx)
                .rows(self.rows.unwrap_or(1))
                .h(if self.state.read(cx).mode == InputMode::Multiline {
                    px(24. * self.rows.unwrap_or(1) as f32 + 22.)
                } else {
                    px(44.)
                })
                .text_size(px(16.))
                .px(px(14.))
        })
    }
}

fn open_input(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Context<InputState>) -> InputState + 'static,
) -> gpui::WindowHandle<TestInput> {
    cx.update(|cx| {
        crate::components::input::init(cx);
        crate::theme::runtime::ThemeRuntime::install(
            crate::theme::ThemeStore::new(crate::default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
    });
    cx.open_window(size(px(220.), px(180.)), move |_, cx| {
        let state = cx.new(build);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_for_subscription = changes.clone();
        let subscription = cx.subscribe(&state, move |_, _, event, _| {
            if let InputEvent::Change(value) = event {
                changes_for_subscription.borrow_mut().push(value.clone());
            }
        });
        TestInput {
            state,
            rows: None,
            changes,
            _subscription: subscription,
        }
    })
}

fn open_input_with_rows(
    cx: &mut TestAppContext,
    rows: usize,
    build: impl FnOnce(&mut Context<InputState>) -> InputState + 'static,
) -> gpui::WindowHandle<TestInput> {
    cx.update(|cx| {
        crate::components::input::init(cx);
        crate::theme::runtime::ThemeRuntime::install(
            crate::theme::ThemeStore::new(crate::default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
    });
    cx.open_window(size(px(220.), px(180.)), move |_, cx| {
        let state = cx.new(build);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_for_subscription = changes.clone();
        let subscription = cx.subscribe(&state, move |_, _, event, _| {
            if let InputEvent::Change(value) = event {
                changes_for_subscription.borrow_mut().push(value.clone());
            }
        });
        TestInput {
            state,
            rows: Some(rows),
            changes,
            _subscription: subscription,
        }
    })
}

fn draw_and_focus(
    window: &gpui::WindowHandle<TestInput>,
    cx: &mut TestAppContext,
) -> VisualTestContext {
    let mut visual = VisualTestContext::from_window((*window).into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    window
        .update(&mut visual.cx, |view, window, cx| {
            let focus_handle = view.state.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
        })
        .unwrap();
    visual
}

#[gpui::test]
fn double_click_selects_the_complete_scrolled_single_line(cx: &mut TestAppContext) {
    let value = "前😀 A deliberately long single-line value extending beyond the viewport";
    let window = open_input(cx, move |cx| InputState::new(cx).initial_value(value));
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    let position = window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert!(input.single_line_scroll_offset < px(0.));
            input.last_viewport_bounds.unwrap().center()
        })
        .unwrap();
    visual.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        modifiers: Default::default(),
        first_mouse: false,
    });
    visual.simulate_mouse_move(position, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(position, MouseButton::Left, Default::default());
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 0..value.len());
            assert!(!input.is_selecting);
            assert!(input.selection_line_anchor.is_none());
        })
        .unwrap();
    visual.simulate_keystrokes("x");
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "x");
        })
        .unwrap();
}

#[gpui::test]
fn double_click_drag_extends_and_reverses_by_whole_lines(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 4, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("前😀\n中间\n末尾")
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    let positions = window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            let layout = input.last_layout.as_ref().unwrap();
            [0, 8, 15].map(|offset| {
                input.last_bounds.unwrap().origin
                    + layout.position_for_offset(offset)
                    + point(px(1.), layout.line_height / 2.)
            })
        })
        .unwrap();
    visual.simulate_event(MouseDownEvent {
        position: positions[1],
        button: MouseButton::Left,
        click_count: 2,
        modifiers: Default::default(),
        first_mouse: false,
    });
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).selected_range, 8..14);
        })
        .unwrap();
    visual.simulate_mouse_move(positions[2], MouseButton::Left, Default::default());
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 8..21);
            assert!(!input.selection_reversed);
        })
        .unwrap();
    visual.simulate_mouse_move(positions[0], MouseButton::Left, Default::default());
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 0..14);
            assert!(input.selection_reversed);
        })
        .unwrap();
    visual.simulate_mouse_up(positions[0], MouseButton::Left, Default::default());
    visual.simulate_mouse_down(positions[1], MouseButton::Left, Default::default());
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert!(input.selected_range.is_empty());
            assert!(input.selection_line_anchor.is_none());
        })
        .unwrap();
    visual.simulate_mouse_up(positions[1], MouseButton::Left, Default::default());
}

#[gpui::test]
fn double_click_selects_the_logical_line_across_soft_wraps(cx: &mut TestAppContext) {
    let value = "A long line with 中文 and 😀 that wraps across several visual rows.";
    let window = open_input_with_rows(cx, 8, move |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value(format!("{value}\nnext"))
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    let position = window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            let layout = input.last_layout.as_ref().unwrap();
            assert!(!layout.lines[0].wrap_boundaries().is_empty());
            input.last_bounds.unwrap().origin + point(px(10.), layout.line_height * 1.5)
        })
        .unwrap();
    visual.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        modifiers: Default::default(),
        first_mouse: false,
    });
    visual.simulate_mouse_up(position, MouseButton::Left, Default::default());
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).selected_range, 0..value.len());
        })
        .unwrap();
}

#[gpui::test]
fn double_click_drag_stops_when_the_value_changes(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀hello"));
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    let position = window
        .update(&mut visual.cx, |view, _, cx| {
            view.state.read(cx).last_viewport_bounds.unwrap().center()
        })
        .unwrap();
    for external_update in [false, true] {
        visual.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Default::default(),
            first_mouse: false,
        });
        if external_update {
            window
                .update(&mut visual.cx, |view, _, cx| {
                    view.state.update(cx, |input, cx| input.set_value("短", cx));
                })
                .unwrap();
        } else {
            visual.simulate_keystrokes("x");
        }
        visual.simulate_mouse_move(position, MouseButton::Left, Default::default());
        window
            .update(&mut visual.cx, |view, _, cx| {
                let input = view.state.read(cx);
                assert_eq!(
                    input.value().as_ref(),
                    if external_update { "短" } else { "x" }
                );
                assert_eq!(
                    input.selected_range,
                    input.content.len()..input.content.len()
                );
                assert!(!input.is_selecting);
                assert!(input.selection_line_anchor.is_none());
            })
            .unwrap();
        visual.simulate_mouse_up(position, MouseButton::Left, Default::default());
    }
}

#[gpui::test]
fn multiline_enter_inserts_newline(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| {
        InputState::new(cx).multiline().initial_value("first")
    });
    let mut visual = draw_and_focus(&window, cx);

    visual.simulate_keystrokes("enter");

    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "first\n");
        })
        .unwrap();
}

#[gpui::test]
fn multiline_vertical_navigation_uses_visual_rows(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| {
        InputState::new(cx).multiline().initial_value("abc\ndef")
    });
    let mut visual = draw_and_focus(&window, cx);

    visual.simulate_keystrokes("up enter");

    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "abc\n\ndef");
        })
        .unwrap();
}

#[gpui::test]
fn multiline_soft_wraps_to_the_available_width(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("A deliberately long line that must wrap inside the input.")
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        visual.debug_bounds("fusion-input-scrollbar").is_some(),
        "a multiline input should render its scrollbar"
    );

    window
        .update(&mut visual.cx, |view, _, cx| {
            assert!(
                view.state
                    .read(cx)
                    .last_layout
                    .as_ref()
                    .unwrap()
                    .visual_row_count()
                    > 1
            );
        })
        .unwrap();
}

#[gpui::test]
fn multiline_content_taller_than_the_viewport_is_scrollable(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight")
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            let scroll_handle = &view.state.read(cx).scroll_handle;
            assert!(scroll_handle.max_offset().y > px(0.));
            assert!(scroll_handle.offset().y < px(0.));
        })
        .unwrap();

    window
        .update(&mut visual.cx, |view, _, cx| {
            view.state
                .read(cx)
                .scroll_handle
                .set_offset(point(px(0.), px(0.)));
            cx.notify();
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).scroll_handle.offset().y, px(0.));
        })
        .unwrap();
}

#[gpui::test]
fn single_line_keeps_a_long_value_and_its_cursor_visible(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| {
        InputState::new(cx)
            .initial_value("A deliberately long single-line value that exceeds the input width")
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert!(input.single_line_scroll_offset < px(0.));

            let layout = input.last_layout.as_ref().unwrap();
            let bounds = input.last_bounds.unwrap();
            let viewport = input.last_viewport_bounds.unwrap();
            let cursor = layout.position_for_offset(input.cursor_offset());
            let cursor_right = bounds.left() + cursor.x + crate::components::input::field::length(&input.caret_style.width, px(1.5));
            assert!(
                cursor_right <= viewport.right(),
                "cursor_right={cursor_right:?}, viewport={viewport:?}, offset={:?}, bounds={bounds:?}, cursor={cursor:?}",
                input.single_line_scroll_offset,
            );
            assert!(
                input.index_for_mouse_position(point(
                    viewport.left() + px(1.),
                    viewport.top() + viewport.size.height / 2.,
                )) > 0,
                "mouse hit testing must account for the shifted text origin",
            );
        })
        .unwrap();

    visual.simulate_keystrokes("home");
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.cursor_offset(), 0);
            assert_eq!(input.single_line_scroll_offset, px(0.));
        })
        .unwrap();
}

#[gpui::test]
fn single_line_drag_selection_scrolls_outside_both_edges_and_stops_on_release(
    cx: &mut TestAppContext,
) {
    let window = open_input(cx, |cx| {
        InputState::new(cx)
        .initial_value("长文本 Unicode selection stays anchored while dragging beyond either edge of this narrow input")
    });
    let mut visual = draw_and_focus(&window, cx);
    let snapshot = |visual: &mut VisualTestContext| {
        window
            .update(&mut visual.cx, |view, _, cx| {
                let input = view.state.read(cx);
                (
                    input.single_line_scroll_offset,
                    input.selected_range.clone(),
                )
            })
            .unwrap()
    };
    for left in [true, false] {
        visual.simulate_keystrokes(if left { "end" } else { "home" });
        visual.update(|window, cx| window.draw(cx).clear());
        let viewport = window
            .update(&mut visual.cx, |view, _, cx| {
                view.state.read(cx).last_viewport_bounds.unwrap()
            })
            .unwrap();
        let down = viewport.center();
        visual.simulate_mouse_down(down, MouseButton::Left, gpui::Modifiers::default());
        let anchor = snapshot(&mut visual).1.start;
        let outside = point(
            if left {
                viewport.left() - px(30.)
            } else {
                viewport.right() + px(30.)
            },
            viewport.top() - px(12.),
        );
        visual.simulate_mouse_move(outside, MouseButton::Left, gpui::Modifiers::default());
        visual.update(|window, cx| window.draw(cx).clear());
        let before = snapshot(&mut visual);
        // No further mouse events: holding at the edge must keep extending the selection.
        for _ in 0..4 {
            visual
                .cx
                .executor()
                .advance_clock(Duration::from_millis(16));
            visual.run_until_parked();
            visual.update(|window, cx| window.draw(cx).clear());
        }
        let after = snapshot(&mut visual);
        if left {
            assert!(after.0 > before.0, "left drag must reveal earlier text");
            assert!(after.1.start < before.1.start);
            assert_eq!(after.1.end, anchor);
        } else {
            assert!(after.0 < before.0, "right drag must reveal later text");
            assert!(after.1.end > before.1.end);
            assert_eq!(after.1.start, anchor);
        }
        visual.simulate_mouse_up(outside, MouseButton::Left, gpui::Modifiers::default());
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(100));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear());
        assert_eq!(snapshot(&mut visual), after);
        window
            .update(&mut visual.cx, |view, _, cx| {
                let input = view.state.read(cx);
                assert!(!input.is_selecting);
                assert!(input.selection_scroll_task.is_none());
                assert!(input.content.is_char_boundary(input.selected_range.start));
                assert!(input.content.is_char_boundary(input.selected_range.end));
            })
            .unwrap();
    }
}

#[gpui::test]
fn single_line_drag_scroll_stops_inside_at_content_start_and_when_disabled(
    cx: &mut TestAppContext,
) {
    let window = open_input(cx, |cx| {
        InputState::new(cx).initial_value(
            "A long single line which must scroll left until its very first character is selected",
        )
    });
    let mut visual = draw_and_focus(&window, cx);
    visual.update(|window, cx| window.draw(cx).clear());
    let viewport = window
        .update(&mut visual.cx, |view, _, cx| {
            view.state.read(cx).last_viewport_bounds.unwrap()
        })
        .unwrap();
    let outside = point(viewport.left() - px(60.), viewport.center().y);
    visual.simulate_mouse_down(
        viewport.center(),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    visual.simulate_mouse_move(outside, MouseButton::Left, gpui::Modifiers::default());
    visual.simulate_mouse_move(
        viewport.center(),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert!(view.state.read(cx).selection_scroll_task.is_none());
        })
        .unwrap();
    visual.simulate_mouse_move(outside, MouseButton::Left, gpui::Modifiers::default());
    for _ in 0..100 {
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(16));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.single_line_scroll_offset, px(0.));
            assert_eq!(input.selected_range.start, 0);
            assert!(input.selection_scroll_task.is_none());
        })
        .unwrap();
    visual.simulate_mouse_move(
        point(viewport.right() + px(30.), viewport.center().y),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(&mut visual.cx, |view, _, cx| {
            view.state.update(cx, |input, cx| {
                assert!(input.selection_scroll_task.is_some());
                input.set_disabled(true, cx);
                assert!(input.selection_scroll_task.is_none());
                assert!(!input.is_selecting);
            });
        })
        .unwrap();
}

#[gpui::test]
fn multiline_scrollbar_stays_at_the_edge_and_dragging_preserves_selection(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 3, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight")
    });
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, _, cx| {
            view.state.update(cx, |input, cx| input.move_to(0, cx));
        })
        .unwrap();
    for _ in 0..3 {
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
    }
    let track = visual.debug_bounds("fusion-input-scrollbar").unwrap();
    window
        .update(&mut visual.cx, |view, _, cx| {
            let viewport = view.state.read(cx).scroll_handle.bounds();
            assert!(track.left() >= viewport.right());
            assert!(track.top() < viewport.top());
            assert!(track.bottom() > viewport.bottom());
        })
        .unwrap();
    let start = point(track.center().x, track.top() + px(8.));
    let end = point(track.center().x, track.bottom() - px(8.));
    visual.simulate_mouse_down(start, MouseButton::Left, gpui::Modifiers::default());
    visual.simulate_mouse_move(end, MouseButton::Left, gpui::Modifiers::default());
    visual.simulate_mouse_up(end, MouseButton::Left, gpui::Modifiers::default());
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert!(input.scroll_handle.offset().y < px(0.));
            assert_eq!(input.selected_range, 0..0);
            assert!(!input.is_selecting);
            assert!(!input.scrollbar_state.is_dragging());
        })
        .unwrap();
}

fn assert_cursor_row_visible(input: &InputState) {
    let layout = input.last_layout.as_ref().unwrap();
    let top = input.last_bounds.unwrap().top()
        + layout
            .position_for_caret(input.cursor_offset(), input.caret_affinity)
            .y;
    let viewport = input.scroll_handle.bounds();
    assert!(
        top >= viewport.top(),
        "cursor top {top:?}, viewport {viewport:?}"
    );
    assert!(
        top + layout.line_height <= viewport.bottom(),
        "cursor bottom {:?}, viewport {viewport:?}",
        top + layout.line_height
    );
}

#[gpui::test]
fn multiline_resize_follows_a_visible_cursor_through_reflow(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 4, |cx| {
        InputState::new(cx).multiline().initial_value(
            (0..12)
                .map(|i| format!("paragraph {i}: abcdefghijklmnopqrstuvwxyz\n"))
                .collect::<String>(),
        )
    });
    let mut visual = draw_and_focus(&window, cx);
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_cursor_row_visible(view.state.read(cx));
            view.rows = Some(2);
            cx.notify();
        })
        .unwrap();
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    let rows_before = window
        .update(&mut visual.cx, |view, _, cx| {
            assert_cursor_row_visible(view.state.read(cx));
            view.state
                .read(cx)
                .last_layout
                .as_ref()
                .unwrap()
                .visual_row_count()
        })
        .unwrap();
    visual.simulate_resize(size(px(164.), px(180.)));
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_cursor_row_visible(view.state.read(cx));
            assert!(
                view.state
                    .read(cx)
                    .last_layout
                    .as_ref()
                    .unwrap()
                    .visual_row_count()
                    > rows_before
            );
            assert!(view.changes.borrow().is_empty());
        })
        .unwrap();
    let offset = window
        .update(&mut visual.cx, |view, window, cx| {
            window.blur();
            view.rows = Some(1);
            cx.notify();
            view.state.read(cx).scroll_handle.offset()
        })
        .unwrap();
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).scroll_handle.offset(), offset);
        })
        .unwrap();
}

#[gpui::test]
fn multiline_resize_preserves_manual_scroll_away_from_cursor(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 4, |cx| {
        InputState::new(cx).multiline().initial_value(
            (0..20)
                .map(|i| format!("entry {i}: abcdefghijklmnopqrstuvwxyz\n"))
                .collect::<String>(),
        )
    });
    let mut visual = draw_and_focus(&window, cx);
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_cursor_row_visible(input);
            input.scroll_handle.set_offset(point(px(0.), px(-24.)));
            view.rows = Some(2);
            cx.notify();
        })
        .unwrap();
    visual.simulate_resize(size(px(164.), px(180.)));
    for _ in 0..4 {
        visual.update(|window, cx| window.draw(cx).clear());
    }
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.scroll_handle.offset().y, px(-24.));
            let layout = input.last_layout.as_ref().unwrap();
            let top = input.last_bounds.unwrap().top()
                + layout.position_for_offset(input.cursor_offset()).y;
            assert!(top > input.scroll_handle.bounds().bottom());
            assert!(!input.scroll_cursor_pending);
        })
        .unwrap();
}

#[gpui::test]
fn multiline_enter_scrolls_as_soon_as_the_cursor_adds_a_row(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 3, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("one\ntwo\nthree")
    });
    let mut visual = draw_and_focus(&window, cx);

    visual.simulate_keystrokes("enter");
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.value().as_ref(), "one\ntwo\nthree\n");
            assert!(input.scroll_handle.max_offset().y > px(0.));
            assert!(input.scroll_handle.offset().y < px(0.));
        })
        .unwrap();

    visual.simulate_keystrokes("enter");
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            let layout = input.last_layout.as_ref().unwrap();
            let bounds = input.last_bounds.unwrap();
            let cursor = layout.position_for_offset(input.cursor_offset());
            let cursor_bottom = bounds.top() + cursor.y + layout.line_height;
            assert_eq!(input.value().as_ref(), "one\ntwo\nthree\n\n");
            assert!(
                cursor_bottom <= input.scroll_handle.bounds().bottom(),
                "cursor_bottom={cursor_bottom:?}, viewport={:?}, offset={:?}, max_offset={:?}, bounds={bounds:?}, cursor={cursor:?}",
                input.scroll_handle.bounds(),
                input.scroll_handle.offset(),
                input.scroll_handle.max_offset(),
            );
        })
        .unwrap();

    visual.simulate_keystrokes("backspace backspace");
    visual.update(|window, cx| window.draw(cx).clear());
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.value().as_ref(), "one\ntwo\nthree");
            assert_eq!(input.scroll_handle.max_offset().y, px(0.));
            assert_eq!(input.scroll_handle.offset().y, px(0.));
            assert_eq!(input.last_layout.as_ref().unwrap().visual_row_count(), 3);
        })
        .unwrap();
}

#[gpui::test]
fn repeated_select_all_keeps_every_multiline_row_selected(cx: &mut TestAppContext) {
    let window = open_input_with_rows(cx, 3, |cx| {
        InputState::new(cx)
            .multiline()
            .initial_value("one\ntwo\nthree")
    });
    let mut visual = draw_and_focus(&window, cx);

    let select_all = &gpui::platform_shortcut("ctrl-a");
    visual.simulate_keystrokes(select_all);
    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 0..input.content.len());
        })
        .unwrap();
    visual.simulate_keystrokes(select_all);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 0..input.content.len());
            let quads = super::super::element::selection_quads(
                input.last_layout.as_ref().unwrap(),
                input.selected_range.clone(),
                input.last_bounds.unwrap(),
                crate::components::input::field::color(&input.selection_style.background),
            );
            assert_eq!(quads.len(), 3);
        })
        .unwrap();
}

#[gpui::test]
fn single_line_enter_does_not_insert_newline(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("first"));
    let mut visual = draw_and_focus(&window, cx);

    visual.simulate_keystrokes("enter");

    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "first");
        })
        .unwrap();
}

#[gpui::test]
fn system_selection_uses_utf16_and_preserves_direction(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀后"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.set_selected_text_range(std::ops::Range { start: 3, end: 1 }, window, cx);
                let selection = input.selected_text_range(false, window, cx).unwrap();
                assert_eq!(selection.range, 1..3);
                assert!(selection.reversed);
                input.replace_text_in_range(None, "X", window, cx);
                assert_eq!(input.value().as_ref(), "前X后");
                input.disabled = true;
                input.set_selected_text_range(0..usize::MAX, window, cx);
                assert_eq!(
                    input.selected_text_range(false, window, cx).unwrap().range,
                    2..2
                );
                assert!(!input.accepts_text_input(window, cx));
            });
        })
        .unwrap();
}

#[gpui::test]
fn surrounding_deletion_preserves_selected_text_and_direction(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀选中🌍后"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, _| {
                input.selected_range = 7..13;
                input.selection_reversed = true;
            });
            let mut handler = gpui::ElementInputHandler::new(Bounds::default(), view.state.clone());
            let snapshot =
                gpui::InputHandler::surrounding_text(&mut handler, 4000, window, cx).unwrap();
            assert_eq!((snapshot.cursor, snapshot.anchor), (7, 13));
            let (before, after) = snapshot.deletion_utf16(4, 4).unwrap();
            assert!(gpui::InputHandler::delete_surrounding_text(
                &mut handler,
                before,
                after,
                window,
                cx
            ));
            let input = view.state.read(cx);
            assert_eq!(input.value().as_ref(), "前选中后");
            assert_eq!(input.selected_range, 3..9);
            assert_eq!(input.cursor_offset(), 3);
            assert!(input.selection_reversed);
        })
        .unwrap();
}

#[gpui::test]
fn surrounding_delete_commit_and_preedit_keep_the_insertion_point(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀后"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.selected_range = 7..7;
                input.replace_and_mark_text_in_range(None, "ni", Some(1..1), window, cx);
                let snapshot = input.surrounding_text(4000, window, cx).unwrap();
                assert_eq!(snapshot.text, "前😀后");
                assert_eq!(snapshot.cursor, 7);
                let deletion = snapshot.deletion_utf16(4, 3).unwrap();
                let marked = input.marked_text_range(window, cx).unwrap();
                input.replace_and_mark_text_in_range(Some(marked), "", None, window, cx);
                assert!(input.delete_surrounding_text(deletion.0, deletion.1, window, cx));
                input.replace_text_in_range(None, "你", window, cx);
                let committed = input.surrounding_text(4000, window, cx).unwrap();
                assert_eq!(committed.text, "前你");
                assert_eq!(committed.cursor, 6);
                input.replace_and_mark_text_in_range(None, "hao", Some(1..1), window, cx);
                assert_eq!(input.value().as_ref(), "前你hao");
                assert_eq!(input.cursor_offset(), 7);
                assert_eq!(input.surrounding_text(4000, window, cx), Some(committed));
            });
        })
        .unwrap();
}

#[gpui::test]
fn surrounding_rejects_invalid_deletions_and_private_fields(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀"));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                assert!(!input.delete_surrounding_text(1, 0, window, cx));
                assert!(!input.delete_surrounding_text(0, 1, window, cx));
                assert!(!input.delete_surrounding_text(4, 0, window, cx));
                assert_eq!(input.value().as_ref(), "前😀");
                assert_eq!(input.selected_range, 7..7);
                input.mode = InputMode::Password;
                assert!(input.surrounding_text(4000, window, cx).is_none());
                assert!(!input.delete_surrounding_text(2, 0, window, cx));
                input.mode = InputMode::Text;
                input.disabled = true;
                assert!(input.surrounding_text(4000, window, cx).is_none());
                assert!(!input.delete_surrounding_text(2, 0, window, cx));
                assert_eq!(input.value().as_ref(), "前😀");
            });
        })
        .unwrap();
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, _, _| {
            assert!(view.changes.borrow().is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn ime_preedit_selection_uses_inserted_text_offsets(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀ab "));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "ab😀你", Some(2..4), window, cx);
                assert_eq!(input.value().as_ref(), "前😀ab ab😀你");
                assert_eq!(input.selected_range, 12..16);
                assert_eq!(input.marked_range, Some(10..19));
                assert!(!input.selection_reversed);
                input.replace_and_mark_text_in_range(None, "a\r\n😀", Some(3..5), window, cx);
                assert_eq!(input.value().as_ref(), "前😀ab a 😀");
                assert_eq!(input.selected_range, 12..16);
            });
        })
        .unwrap();
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, _, _| {
            assert!(view.changes.borrow().is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn ime_directed_and_hidden_selection_reaches_the_input(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("前😀ab "));
    let mut visual = draw_and_focus(&window, cx);
    window
        .update(&mut visual.cx, |view, window, cx| {
            let mut handler = gpui::ElementInputHandler::new(Bounds::default(), view.state.clone());
            gpui::InputHandler::replace_and_mark_text_with_selection(
                &mut handler,
                None,
                "ab😀你",
                PreeditSelection::Range { anchor: 4, head: 2 },
                window,
                cx,
            );
            let input = view.state.read(cx);
            assert_eq!(input.selected_range, 12..16);
            assert_eq!(input.cursor_offset(), 12);
            assert!(input.selection_reversed);
            gpui::InputHandler::replace_and_mark_text_with_selection(
                &mut handler,
                None,
                "ab😀你",
                PreeditSelection::Hidden,
                window,
                cx,
            );
            let input = view.state.read(cx);
            assert!(input.preedit_cursor_hidden);
            assert!(input.selected_range.is_empty());
            gpui::InputHandler::replace_text_in_range(&mut handler, None, "选", window, cx);
            let input = view.state.read(cx);
            assert_eq!(input.value().as_ref(), "前😀ab 选");
            assert_eq!(input.marked_range, None);
            assert!(!input.selection_reversed);
        })
        .unwrap();
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, _, _| {
            assert_eq!(
                view.changes.borrow().as_slice(),
                &[SharedString::from("前😀ab 选")]
            );
        })
        .unwrap();
}

#[gpui::test]
fn ime_preedit_is_visible_without_emitting_change(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("prefix "));
    let mut visual = draw_and_focus(&window, cx);

    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "ni", None, window, cx);
                input.replace_and_mark_text_in_range(None, "你", None, window, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            let input = view.state.read(cx);
            assert_eq!(input.value().as_ref(), "prefix 你");
            assert_eq!(input.marked_range, Some(7..10));
            assert!(view.changes.borrow().is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn ime_candidate_commit_emits_one_change(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("prefix "));
    let mut visual = draw_and_focus(&window, cx);

    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "ni", None, window, cx);
                input.replace_and_mark_text_in_range(None, "你", None, window, cx);
                input.replace_text_in_range(None, "你", window, cx);
                input.unmark_text(window, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, _| {
            assert_eq!(
                view.changes.borrow().as_slice(),
                &[SharedString::from("prefix 你")]
            );
        })
        .unwrap();
}

#[gpui::test]
fn ime_unmark_commits_when_the_platform_does_not_insert_again(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("prefix "));
    let mut visual = draw_and_focus(&window, cx);

    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "かな", None, window, cx);
                input.unmark_text(window, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, _| {
            assert_eq!(
                view.changes.borrow().as_slice(),
                &[SharedString::from("prefix かな")]
            );
        })
        .unwrap();
}

#[gpui::test]
fn ime_remarking_and_surrounding_edits_do_not_commit_candidates(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("A😀Z"));
    let mut visual = draw_and_focus(&window, cx);

    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(Some(1..1), "ni", None, window, cx);
                // Moving the composing span must not finish the previous one.
                input.replace_and_mark_text_in_range(Some(1..2), "n", None, window, cx);
                input.replace_and_mark_text_in_range(Some(1..3), "ni", None, window, cx);
                // Delete A and the emoji while retaining the candidate between them.
                input.replace_and_mark_text_in_range(Some(0..5), "ni", None, window, cx);
                input.replace_and_mark_text_in_range(Some(0..2), "ni", None, window, cx);
            });
        })
        .unwrap();
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, window, cx| {
            assert!(view.changes.borrow().is_empty());
            assert_eq!(view.state.read(cx).value().as_ref(), "niZ");
            view.state.update(cx, |input, cx| {
                input.replace_text_in_range(None, "", window, cx);
            });
        })
        .unwrap();
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "Z");
            assert_eq!(view.changes.borrow().as_slice(), &[SharedString::from("Z")]);
        })
        .unwrap();
}

#[gpui::test]
fn cancelling_ime_preedit_does_not_emit_a_change(cx: &mut TestAppContext) {
    let window = open_input(cx, |cx| InputState::new(cx).initial_value("prefix "));
    let mut visual = draw_and_focus(&window, cx);

    window
        .update(&mut visual.cx, |view, window, cx| {
            view.state.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "ni", None, window, cx);
                input.replace_text_in_range(None, "", window, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });

    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.state.read(cx).value().as_ref(), "prefix ");
            assert!(view.changes.borrow().is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn readonly_and_disabling_cancel_composition_without_committing(cx: &mut TestAppContext) {
    let view = open_input(cx, |cx| InputState::new(cx).initial_value("Record"));
    let mut visual = draw_and_focus(&view, cx);
    view.update(&mut visual.cx, |view, window, cx| {
        view.state.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "xyz", Some(2..2), window, cx);
            assert_eq!(input.value().as_ref(), "Recordxyz");
            input.set_read_only(true, cx);
            assert_eq!(input.value().as_ref(), "Record");
            assert!(!input.is_composing());
            assert!(!input.accepts_text_input(window, cx));
            input.select_all(&SelectAll, window, cx);
            input.copy(&Copy, window, cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "Record");
            input.cut(&Cut, window, cx);
            input.replace_text_in_range(None, "invalid", window, cx);
            input.replace_and_mark_text_in_range(None, "invalid", None, window, cx);
            assert_eq!(input.value().as_ref(), "Record");
            assert!(!input.is_composing());
            input.set_read_only(false, cx);
            input.move_to(input.content.len(), cx);
            input.replace_and_mark_text_in_range(None, "more", None, window, cx);
            input.set_disabled(true, cx);
            assert_eq!(input.value().as_ref(), "Record");
            assert!(!input.accepts_text_input(window, cx));
            input.set_disabled(false, cx);
            assert!(input.accepts_text_input(window, cx));
        });
    })
    .unwrap();
    visual.run_until_parked();
    view.update(&mut visual.cx, |view, _, _| {
        assert!(view.changes.borrow().is_empty())
    })
    .unwrap();
}

#[gpui::test]
fn history_groups_composition_and_preserves_selection_across_undo(cx: &mut TestAppContext) {
    let view = open_input(cx, |cx| InputState::new(cx).initial_value("abcdef"));
    let mut visual = draw_and_focus(&view, cx);
    view.update(&mut visual.cx, |view, window, cx| {
        view.state.update(cx, |input, cx| {
            input.set_selected_text_range(2..4, window, cx);
            input.replace_and_mark_text_in_range(None, "h", None, window, cx);
            input.replace_and_mark_text_in_range(None, "hh", None, window, cx);
            input.replace_text_in_range(None, "界", window, cx);
            assert_eq!(input.value().as_ref(), "ab界ef");
            input.undo(&Undo, window, cx);
            assert_eq!(input.value().as_ref(), "abcdef");
            assert_eq!(input.selected_range, 2..4);
            input.redo(&Redo, window, cx);
            assert_eq!(input.value().as_ref(), "ab界ef");
            assert_eq!(input.selected_range, 5..5);
            input.undo(&Undo, window, cx);
            input.replace_text_in_range(None, "Q", window, cx);
            input.redo(&Redo, window, cx);
            assert_eq!(input.value().as_ref(), "abQef");
            input.set_read_only(true, cx);
            input.undo(&Undo, window, cx);
            assert_eq!(input.value().as_ref(), "abQef");
            input.set_read_only(false, cx);
            input.set_value("replacement", cx);
            input.undo(&Undo, window, cx);
            assert_eq!(input.value().as_ref(), "replacement");
            input.set_mode(InputMode::Password, cx);
            input.replace_text_in_range(None, "private", window, cx);
            input.undo(&Undo, window, cx);
            assert_eq!(input.value().as_ref(), "replacementprivate");
        });
    })
    .unwrap();
}

#[gpui::test]
fn password_masks_graphemes_and_maps_pointer_and_ime_to_source(cx: &mut TestAppContext) {
    let view = open_input(cx, |cx| {
        InputState::new(cx).password().initial_value("é👩‍🚀k\u{301}")
    });
    let mut visual = draw_and_focus(&view, cx);
    view.update(&mut visual.cx, |view, window, cx| {
        view.state.update(cx, |input, cx| {
            let bounds = input.last_bounds.unwrap();
            assert_eq!(input.password_mask().as_ref(), "•••");
            let display = input.last_layout.as_ref().unwrap();
            assert_eq!(display.lines[0].text.as_ref(), "•••");
            let boundary = display.position_for_offset('•'.len_utf8());
            let pointer = bounds.origin + boundary;
            let caret = input.caret_for_mouse_position(pointer);
            assert_eq!(caret.index, "é".len());
            assert_eq!(
                input.character_index_for_point(pointer, window, cx),
                Some(1)
            );
            let ime = input.bounds_for_range(1..1, bounds, window, cx).unwrap();
            assert_eq!(ime.left(), pointer.x);
            input.focus_at(pointer, window, cx);
            input.replace_text_in_range(None, "R", window, cx);
            assert_eq!(input.value().as_ref(), "éR👩‍🚀k\u{301}");
            input.backspace(&Backspace, window, cx);
            assert_eq!(input.value().as_ref(), "é👩‍🚀k\u{301}");
            input.right(&Right, window, cx);
            input.backspace(&Backspace, window, cx);
            assert_eq!(input.value().as_ref(), "ék\u{301}");
        });
    })
    .unwrap();
}

#[gpui::test]
fn password_copy_tracks_visibility_and_keeps_readonly_and_disabled_rules(cx: &mut TestAppContext) {
    let view = open_input(cx, |cx| {
        InputState::new(cx).password().initial_value("aλ🪶z")
    });
    let mut visual = draw_and_focus(&view, cx);
    view.update(&mut visual.cx, |view, window, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into()));
        view.state.update(cx, |input, cx| {
            input.set_selected_text_range(1..4, window, cx);
        });
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-c");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "unchanged"
        );
        view.state
            .update(cx, |input, cx| input.set_password_visible(true, cx));
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-c");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "λ🪶");
        cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into()));
        view.state
            .update(cx, |input, cx| input.set_read_only(true, cx));
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-c");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "λ🪶");
        cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into()));
        view.state
            .update(cx, |input, cx| input.set_password_visible(false, cx));
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-c");
    view.update(&mut visual.cx, |view, window, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "unchanged"
        );
        view.state.update(cx, |input, cx| {
            input.set_password_visible(true, cx);
            input.set_disabled(true, cx);
            input.copy(&Copy, window, cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "unchanged"
            );
            assert_eq!(input.value().as_ref(), "aλ🪶z");
        });
    })
    .unwrap();
}

#[gpui::test]
fn masked_password_ime_is_opt_in_without_disabling_direct_input(cx: &mut TestAppContext) {
    let view = open_input(cx, |cx| InputState::new(cx).password());
    let mut visual = draw_and_focus(&view, cx);
    visual.simulate_keystrokes("r");
    view.update(&mut visual.cx, |view, window, cx| {
        let mut handler = gpui::ElementInputHandler::new(Bounds::default(), view.state.clone());
        assert!(gpui::InputHandler::accepts_text_input(
            &mut handler,
            window,
            cx
        ));
        assert!(!gpui::InputHandler::ime_enabled(&mut handler, window, cx));
        assert!(!gpui::InputHandler::prefers_ime_for_printable_keys(
            &mut handler,
            window,
            cx
        ));
        view.state.update(cx, |input, cx| {
            assert_eq!(input.value().as_ref(), "r");
            input.replace_and_mark_text_in_range(None, "xy", Some(2..2), window, cx);
            assert!(!input.is_composing());
            input.replace_and_mark_text_with_selection(
                None,
                "xy",
                PreeditSelection::Hidden,
                window,
                cx,
            );
            assert!(!input.preedit_cursor_hidden);
            assert_eq!(input.value().as_ref(), "r");
            input.set_password_ime(true, cx);
            assert!(input.ime_enabled(window, cx));
            input.replace_and_mark_text_in_range(None, "xy", Some(2..2), window, cx);
            assert!(input.is_composing());
            input.set_password_ime(false, cx);
            assert!(!input.is_composing());
            assert_eq!(input.value().as_ref(), "r");
            input.set_password_visible(true, cx);
            assert!(input.ime_enabled(window, cx));
            input.replace_and_mark_text_with_selection(
                None,
                "zw",
                PreeditSelection::Hidden,
                window,
                cx,
            );
            assert!(input.preedit_cursor_hidden);
            assert!(input.is_composing());
            input.set_password_visible(false, cx);
            assert!(!input.is_composing());
            assert!(!input.preedit_cursor_hidden);
            assert_eq!(input.value().as_ref(), "r");
            input.set_password_ime(true, cx);
            input.replace_and_mark_text_in_range(None, "uv", Some(2..2), window, cx);
            input.replace_text_in_range(None, "界", window, cx);
            assert_eq!(input.value().as_ref(), "r界");
            input.set_read_only(true, cx);
            assert!(!input.ime_enabled(window, cx));
            input.set_read_only(false, cx);
            input.set_disabled(true, cx);
            assert!(!input.ime_enabled(window, cx));
        });
    })
    .unwrap();
    visual.run_until_parked();
    view.update(&mut visual.cx, |view, _, _| {
        assert_eq!(
            &*view.changes.borrow(),
            &[SharedString::from("r"), SharedString::from("r界")]
        );
    })
    .unwrap();
}
