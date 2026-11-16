use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        toast::{Toast, ToastHost, ToastId, ToastKind, ToastPlacement},
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    host: Entity<ToastHost>,
    buttons: Vec<Entity<Button>>,
    pending: Option<ToastId>,
    placement: usize,
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
        div().size_full().bg(color("canvas")).text_color(color("foreground")).p_8().flex().flex_col().gap_6()
            .child(div().text_2xl().child("Toast notifications"))
            .child(format!("Material: {:?}",theme.material_mode()))
            .child(div().flex().flex_wrap().gap_3().children(self.buttons.iter().take(3).cloned()))
            .child(div().rounded_xl().bg(color("surface")).p_6().flex().flex_col().gap_5()
                .child("Messages")
                .child(div().flex().flex_wrap().gap_3().children(self.buttons.iter().skip(3).cloned()))
                .child(div().text_sm().text_color(color("foreground_secondary")).child("Hover or focus the close button to pause · Esc closes the focused notification")))
            .child(div().flex_1().rounded_xl().bg(color("primary_subtle")).p_6()
                .child("Notifications float above this content. Start a transfer, then finish it to update the same message."))
            .child(self.host.clone())
    }
}
fn main() {
    gpui_platform::application().with_assets(IconAssets).run(|cx| {
        let theme=ThemeRuntime::install(ThemeStore::new(default_theme(),"light").unwrap(),"neutral",cx).unwrap();
        cx.open_window(WindowOptions {titlebar:None,window_bounds:Some(WindowBounds::Windowed(Bounds::centered(None,size(px(820.),px(760.)),cx))),..Default::default()},move |window,cx|cx.new(|cx| {
            let host=cx.new(|cx|ToastHost::new(window,cx));
            let owner:WeakEntity<Gallery>=cx.weak_entity();
            let buttons=["Toggle theme","Toggle material","Move notifications","Information","Success","Warning","Error","Start transfer","Finish transfer","Queue messages","Clear"]
                .into_iter().enumerate().map(|(index,label)| {
                    let owner=owner.clone();
                    cx.new(|cx|Button::new(label,window,cx).material_mode(MaterialMode::Plain).on_click(move |window,cx| {
                        let _=owner.update(cx,|gallery,cx| {
                            match index {
                                0=>gallery.theme.update(cx,|t,cx|{ t.set_mode(if t.store().mode()=="light" {"dark"} else {"light"},cx).unwrap(); }),
                                1=>gallery.theme.update(cx,|t,cx|t.set_material_mode(if t.material_mode()==MaterialMode::Plain {MaterialMode::Glass} else {MaterialMode::Plain},cx)),
                                2=>{gallery.placement=(gallery.placement+1)%3;gallery.host.update(cx,|h,cx|h.set_placement([ToastPlacement::BottomRight,ToastPlacement::Top,ToastPlacement::Bottom][gallery.placement],cx));},
                                3..=6=>{gallery.host.update(cx,|h,cx| {h.show(match index {
                                    3=>Toast::new("A new revision is available").description("Reload the document when you are ready."),
                                    4=>Toast::new("Snapshot saved").kind(ToastKind::Success),
                                    5=>Toast::new("Storage is nearly full").kind(ToastKind::Warning).description("Remove unused snapshots before the next backup."),
                                    _=>Toast::new("Could not reach the server").kind(ToastKind::Error).description("Your local changes are still available."),
                                },window,cx);});},
                                7=>{ if gallery.pending.is_none_or(|id|!gallery.host.read(cx).contains(id)) {
                                    gallery.pending=Some(gallery.host.update(cx,|h,cx|h.show(Toast::new("Sending archive").kind(ToastKind::Loading).description("The notification stays until the transfer finishes."),window,cx)));
                                } },
                                8=>{if let Some(id)=gallery.pending.take() {gallery.host.update(cx,|h,cx|h.update_toast(id,Toast::new("Archive delivered").kind(ToastKind::Success),window,cx));}},
                                9=>gallery.host.update(cx,|h,cx|{for index in 1..=5 {h.show(Toast::new(format!("Background job {index} finished")).kind(ToastKind::Success),window,cx);}}),
                                _=>gallery.host.update(cx,|h,cx|h.clear(window,cx)),
                            }
                            cx.notify();
                        });
                    }))
                }).collect();
            host.update(cx,|h,cx|{h.show(Toast::new("Notifications are ready").description("Try the message controls above.").timeout(None),window,cx);});
            Gallery {_subscription:cx.observe(&theme,|_,_,cx|cx.notify()),theme,host,buttons,pending:None,placement:0}
        })).unwrap();cx.activate(true);
    });
}
