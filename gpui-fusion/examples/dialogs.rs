use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        dialog::Dialog,
        input::{Input, InputState},
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    dialogs: Vec<Entity<Dialog>>,
    buttons: Vec<Entity<Button>>,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme.read(cx);
        let context = ResolveContext::new(
            theme.store().theme(),
            theme.store().mode(),
            theme.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        div().size_full().bg(color("canvas")).text_color(color("foreground"))
            .p_8().flex().flex_col().gap_6()
            .child(div().text_2xl().child("Dialogs"))
            .child(format!("Material: {:?}", theme.material_mode()))
            .child(div().flex().gap_3().children(self.buttons.iter().take(2).cloned()))
            .child(div().relative().flex_1().rounded_3xl().bg(color("surface")).p_8().overflow_hidden()
                .child(div().absolute().right(px(50.)).top(px(60.)).size(px(260.)).rounded_full().bg(color("primary_subtle")))
                .child(div().absolute().left(px(100.)).bottom(px(60.)).w(px(320.)).h(px(180.)).rounded_3xl().bg(color("muted_surface")))
                .child(div().relative().flex().flex_col().gap_5()
                    .child("Collection settings")
                    .child(div().flex().flex_wrap().gap_3().children(self.buttons.iter().skip(2).cloned()))
                    .child("Tab stays in the dialog · Esc closes · Text fields keep their editing state")))
            .children(self.dialogs.iter().cloned())
    }
}
fn main() {
    gpui_platform::application().with_assets(IconAssets).run(|cx| {
        let theme = ThemeRuntime::install(ThemeStore::new(default_theme(), "light").unwrap(), "neutral", cx).unwrap();
        theme.update(cx, |t, c| t.set_material_mode(MaterialMode::Glass, c));
        cx.open_window(WindowOptions {
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(880.), px(720.)), cx))),
            ..Default::default()
        }, move |window, cx| cx.new(|cx| {
            let name = cx.new(|c| InputState::new(c).placeholder("Collection name"));
            let notes = cx.new(|c| InputState::new(c).multiline().placeholder("Description"));
            let name_field = cx.new(|c| Input::new(&name, window, c));
            let notes_field = cx.new(|c| Input::new(&notes, window, c));
            let initial = name.focus_handle(cx);
            let editor = cx.new(|c: &mut Context<Dialog>| {
                let weak = c.weak_entity();
                let cancel = c.new(|c| Button::new("Cancel", window, c).on_click(move |w, c| {
                    let _ = weak.update(c, |d, c| d.close(w, c));
                }));
                let weak = c.weak_entity();
                let save = c.new(|c| Button::new("Done", window, c).variant("primary").on_click(move |w, c| {
                    let _ = weak.update(c, |d, c| d.close(w, c));
                }));
                Dialog::new("Edit collection", move |_, _| {
                    div().flex().flex_col().gap_4()
                        .child(div().flex().flex_col().gap_2().child("Name").child(name_field.clone()))
                        .child(div().flex().flex_col().gap_2().child("Notes").child(notes_field.clone()))
                }, window, c)
                    .initial_focus(initial)
                    .footer(move |_, _| div().flex().gap_3().child(cancel.clone()).child(save.clone()))
            });
            let confirmation = cx.new(|c: &mut Context<Dialog>| {
                let weak = c.weak_entity();
                let cancel = c.new(|c| Button::new("Keep index", window, c).on_click(move |w, c| { let _ = weak.update(c, |d, c| d.close(w, c)); }));
                let initial = cancel.focus_handle(c);
                let weak = c.weak_entity();
                let remove = c.new(|c| Button::new("Remove", window, c).variant("primary").on_click(move |w, c| { let _ = weak.update(c, |d, c| d.close(w, c)); }));
                Dialog::new("Remove local index?", |_, _| "The index can be rebuilt later. Original files remain in the collection.", window, c)
                    .close_on_backdrop(false).initial_focus(initial)
                    .footer(move |_, _| div().flex().gap_3().child(cancel.clone()).child(remove.clone()))
            });
            let details = cx.new(|c| Dialog::new("Indexed folders", |_, _| {
                div().flex().flex_col().gap_3().children((1..=30).map(|n| div().py_2().child(format!("Collection folder {n:02}"))))
            }, window, c));
            let theme_button = {
                let theme = theme.clone();
                cx.new(|c| Button::new("Toggle theme", window, c).material_mode(MaterialMode::Plain).on_click(move |_, c| theme.update(c, |t, c| {
                    t.set_mode(if t.store().mode() == "light" { "dark" } else { "light" }, c).unwrap();
                })))
            };
            let material_button = {
                let theme = theme.clone();
                cx.new(|c| Button::new("Toggle material", window, c).material_mode(MaterialMode::Plain).on_click(move |_, c| theme.update(c, |t, c| {
                    t.set_material_mode(if t.material_mode() == MaterialMode::Plain { MaterialMode::Glass } else { MaterialMode::Plain }, c);
                })))
            };
            let dialogs = vec![editor, confirmation, details];
            let mut buttons = vec![theme_button, material_button];
            for (dialog, label) in dialogs.iter().zip(["Edit collection", "Remove index", "Browse folders"]) {
                let dialog = dialog.downgrade();
                buttons.push(cx.new(|c| Button::new(label, window, c).on_click(move |w, c| { let _ = dialog.update(c, |d, c| d.open(w, c)); })));
            }
            buttons[2].focus_handle(cx).focus(window, cx);
            dialogs[0].update(cx, |dialog, cx| dialog.open(window, cx));
            Gallery { _subscription: cx.observe(&theme, |_, _, c| c.notify()), theme, dialogs, buttons }
        })).unwrap();
        cx.activate(true);
    });
}
