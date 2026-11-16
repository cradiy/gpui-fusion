use super::*;
use crate::{default_theme, theme::ThemeStore};
use gpui::{AppContext, TestAppContext, VisualTestContext, size};

struct Harness {
    control: Entity<Progress>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.control.clone())
    }
}
#[gpui::test]
fn progress_updates_validate_atomically_and_follow_theme_without_resetting_value(
    cx: &mut TestAppContext,
) {
    let handle = cx.open_window(size(px(420.), px(200.)), |_, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let control = cx.new(|cx| {
            Progress::new("Indexing", Some(0.37), cx)
                .unwrap()
                .show_value(true)
                .animated(false)
        });
        Harness { control, theme }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    let bounds = visual.debug_bounds("fusion-progress").unwrap();
    assert!(visual.debug_bounds("fusion-progress-value").is_some());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |p, cx| {
                assert!(p.set_value(Some(f32::NAN), cx).is_err());
                assert!(p.set_value(Some(f32::INFINITY), cx).is_err());
                assert_eq!(p.value(), Some(0.37));
            });
            h.theme.update(cx, |theme, cx| {
                theme.set_mode("dark", cx).unwrap();
                theme.set_material_mode(MaterialMode::Glass, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(visual.debug_bounds("fusion-progress").unwrap(), bounds);
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |p, cx| {
                assert_eq!(p.value(), Some(0.37));
                assert!(p.resolve(cx).unwrap().track.material.is_some());
                p.set_value(None, cx).unwrap();
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-progress-value").is_none());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |p, cx| {
                p.shape = ProgressShape::Ring;
                p.set_value(Some(2.), cx).unwrap();
                assert_eq!(p.value(), Some(1.));
                p.set_style(
                    StylePatch::default()
                        .w(px(72.))
                        .h(px(72.))
                        .part(crate::style::PartName("fill".into()), |s| {
                            s.bg(gpui::rgb(0x475569))
                        }),
                    cx,
                );
            })
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-progress").unwrap().size,
        size(px(72.), px(72.))
    );
    assert!(visual.debug_bounds("fusion-progress-value").is_some());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |p, cx| {
                p.set_value(Some(-0.5), cx).unwrap();
                assert_eq!(p.value(), Some(0.));
                assert!(p.style_error().is_none());
            })
        })
        .unwrap();
}
