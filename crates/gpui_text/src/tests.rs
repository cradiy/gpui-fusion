use super::*;

const FONT: &[u8] = include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf");

fn database() -> Database {
    let mut db = Database::new();
    db.load_font_data(FONT.to_vec());
    db
}

fn query() -> Query<'static> {
    Query {
        families: &[Family::Name("Lilex")],
        ..Query::default()
    }
}

#[test]
fn shaping_matches_cosmic_for_ligatures_combining_marks_and_scripts() {
    let mut ours = TextSystem::new(database(), CacheLimits::default());
    let mut cosmic = cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), database());
    for text in ["affinity != 48", "A\u{301} Ωμέγα", "Север 27", "ab אב cd"] {
        let actual = ours.shape(text, &query(), 19.).unwrap();
        let attrs = cosmic_text::AttrsList::new(
            &cosmic_text::Attrs::new().family(cosmic_text::Family::Name("Lilex")),
        );
        let shaped = cosmic_text::ShapeLine::new(
            &mut cosmic,
            text,
            &attrs,
            cosmic_text::Shaping::Advanced,
            4,
        );
        let mut output = Vec::new();
        shaped.layout_to_buffer(
            &mut cosmic_text::ShapeBuffer::default(),
            19.,
            None,
            cosmic_text::Wrap::None,
            cosmic_text::Ellipsize::None,
            None,
            &mut output,
            None,
            cosmic_text::Hinting::Disabled,
        );
        let expected = &output[0];
        assert!(
            (actual.width - expected.w).abs() < 0.01,
            "width for {text:?}: {} != {}",
            actual.width,
            expected.w
        );
        let actual: Vec<_> = actual.runs.iter().flat_map(|run| &run.glyphs).collect();
        assert_eq!(actual.len(), expected.glyphs.len(), "{text:?}");
        for (actual, expected) in actual.iter().zip(&expected.glyphs) {
            assert_eq!(
                (actual.id, actual.cluster),
                (expected.glyph_id as u32, expected.start),
                "{text:?}"
            );
            assert!(
                (actual.x - expected.x - expected.x_offset * 19.).abs() < 0.01,
                "position for {text:?}"
            );
            assert!(
                (actual.y + expected.y_offset * 19.).abs() < 0.01,
                "vertical position for {text:?}"
            );
            assert!(
                (actual.advance - expected.w).abs() < 0.01,
                "advance for {text:?}"
            );
            assert!(text.is_char_boundary(actual.cluster));
        }
    }
}

#[test]
fn rejected_fallbacks_do_not_become_retained_fonts() {
    let mut db = database();
    let face = db.faces().next().unwrap().clone();
    for index in 0..96 {
        let mut candidate = face.clone();
        candidate.families[0].0 = format!("Candidate {index}");
        db.push_face_info(candidate);
    }
    let mut system = TextSystem::new(db, CacheLimits::default());
    let text = format!("x{}y", char::MAX);
    let first = system.shape(&text, &query(), 17.).unwrap();
    assert!(first.runs.iter().flat_map(|r| &r.glyphs).any(|g| g.id == 0));
    assert_eq!(system.cache_usage().font_entries, 1);
    assert_eq!(system.cache_usage().font_bytes, FONT.len());
    let second = system.shape(&text, &query(), 17.).unwrap();
    assert_eq!(first.width, second.width);
    assert_eq!(first.runs[0].glyphs, second.runs[0].glyphs);
}

#[test]
fn eviction_preserves_live_layout_rasterization() {
    let mut db = database();
    db.load_font_data(include_bytes!("../../../assets/fonts/lilex/Lilex-Bold.ttf").to_vec());
    let mut system = TextSystem::new(
        db,
        CacheLimits {
            font_entries: 1,
            ..CacheLimits::default()
        },
    );
    let line = system.shape("Quartz", &query(), 24.).unwrap();
    let mut rasterizer = Rasterizer::default();
    let run = &line.runs[0];
    let glyph = run.glyphs[0].id;
    let before = rasterizer.rasterize(run, glyph, 24.).unwrap().unwrap();
    let bold = Query {
        weight: Weight::BOLD,
        ..query()
    };
    system.shape("Granite", &bold, 24.).unwrap();
    assert_eq!(system.cache_usage().font_entries, 1);
    assert!(!system.fonts.contains_key(&run.font_id));
    system.clear_caches();
    let after = rasterizer.rasterize(run, glyph, 24.).unwrap().unwrap();
    assert_eq!(before.data, after.data);
    assert_eq!(before.placement.width, after.placement.width);
}

#[test]
fn oversized_fonts_and_long_clusters_bypass_retention() {
    let mut system = TextSystem::new(
        database(),
        CacheLimits {
            font_bytes: FONT.len() - 1,
            fallback_entries: 2,
            ..CacheLimits::default()
        },
    );
    let text = format!("a{}", "\u{301}".repeat(80));
    let line = system.shape(&text, &query(), 13.).unwrap();
    assert!(!line.runs.is_empty());
    assert_eq!(system.cache_usage().font_entries, 0);
    assert_eq!(system.cache_usage().fallback_entries, 0);
    system.shape("abcdefgh", &query(), 13.).unwrap();
    assert!(system.cache_usage().fallback_entries <= 2);
}

#[test]
fn loading_fonts_invalidates_failed_fallback_queries() {
    let extra = include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
    let primary = swash::FontRef::from_index(FONT, 0).unwrap();
    let fallback = swash::FontRef::from_index(extra, 0).unwrap();
    let ch = (0x20..0x10000)
        .filter_map(char::from_u32)
        .find(|&ch| {
            primary.charmap().map(ch) == 0 && fallback.charmap().map(ch) != 0 && !is_ignorable(ch)
        })
        .expect("Bundled fonts have different coverage");
    let text = ch.to_string();
    let mut system = TextSystem::new(database(), CacheLimits::default());
    let old = system.shape(&text, &query(), 16.).unwrap();
    assert_eq!(
        system.fallback[&(old.runs[0].font_id, text.clone())],
        old.runs[0].font_id
    );
    system.add_font_data(extra.to_vec());
    let new = system.shape(&text, &query(), 16.).unwrap();
    assert_ne!(new.runs[0].font_id, old.runs[0].font_id);
    assert!(new.runs[0].glyphs.iter().all(|g| g.id != 0));
}

#[test]
fn invalid_line_inputs_do_not_load_fonts() {
    let mut system = TextSystem::new(database(), CacheLimits::default());
    for text in ["a\nb", "a\tb", "a\u{2029}b"] {
        assert!(system.shape(text, &query(), 16.).is_err());
    }
    for size in [f32::NAN, f32::INFINITY, 0., -1.] {
        assert!(system.shape("a", &query(), size).is_err());
    }
    assert_eq!(system.cache_usage().font_entries, 0);
}
