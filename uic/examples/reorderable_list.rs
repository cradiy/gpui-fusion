use gpui::{prelude::*, *};
use uic::{
    assets::{LucideAssets, LucideIcons},
    components::{
        reorderable_list::{ReorderHandle, ReorderState, ReorderableList},
        swipe_actions::{SwipeActions, SwipeActionsState, SwipeDirection, SwipeTriggered},
    },
};

struct Track {
    id: usize,
    title: &'static str,
    artist: &'static str,
    saved: bool,
    swipe: Entity<SwipeActionsState>,
}
struct Example {
    order: Entity<ReorderState>,
    tracks: Vec<Track>,
    status: String,
    _subscriptions: Vec<Subscription>,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let order = cx.new(|cx| ReorderState::new(window, cx));
        let names = [
            ("Soft focus", "North Coast"),
            ("Morning tide", "Low Season"),
            ("A little further", "Paper Planes"),
            ("Blue hour", "Slow Company"),
            ("Open windows", "Daylight"),
            ("In between", "The Quiet Room"),
            ("Stay awhile", "North Coast"),
            ("Homeward", "Paper Planes"),
            ("Golden", "Low Season"),
            ("After the rain", "Daylight"),
            ("Another day", "Slow Company"),
            ("Last light", "The Quiet Room"),
        ];
        let mut subscriptions = vec![];
        let tracks = names
            .into_iter()
            .enumerate()
            .map(|(id, (title, artist))| {
                let swipe = cx.new(|cx| SwipeActionsState::new(window, cx));
                subscriptions.push(cx.subscribe(
                    &swipe,
                    move |this, _, event: &SwipeTriggered, cx| {
                        if let Some(row) = this
                            .tracks
                            .iter_mut()
                            .find(|row| row.id == id)
                        {
                            row.saved = event.direction == SwipeDirection::Right;
                            this.status = format!(
                                "{} · {}",
                                if row.saved { "Saved" } else { "Unsaved" },
                                row.title
                            );
                            cx.notify();
                        }
                    },
                ));
                Track {
                    id,
                    title,
                    artist,
                    saved: false,
                    swipe,
                }
            })
            .collect();
        Self {
            order,
            tracks,
            status: "Make room for your favorites.".into(),
            _subscriptions: subscriptions,
        }
    }
}
impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().safe_area;
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let mut list = ReorderableList::new("queue", &self.order)
            .flex_1()
            .min_h_0()
            .gap_3()
            .p_1()
            .on_reorder(cx.listener(
                |this, event: &uic::components::reorderable_list::ReorderEvent, _, cx| {
                    if this
                        .tracks
                        .get(event.from)
                        .is_some_and(|track| ElementId::from(track.id) == event.id)
                    {
                        let track = this.tracks.remove(event.from);
                        this.status = format!("{} moved to {}", track.title, event.to + 1);
                        this.tracks.insert(event.to, track);
                        cx.notify();
                    }
                },
            ));
        for (index, track) in self.tracks.iter().enumerate() {
            let dragging = self.order.read(cx).dragged_item() == Some(&ElementId::from(track.id));
            let accent = [0x526d91, 0x528078, 0xa47867][track.id % 3];
            let row = div()
                .w_full()
                .h(px(if track.id == 2 { 102. } else { 82. }))
                .px_3()
                .rounded_xl()
                .bg(rgb(if dragging { 0xe8f0ff } else { 0xffffff }))
                .border_1()
                .border_color(rgb(if dragging { 0x648be6 } else { 0xe8edf4 }))
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(44.))
                        .flex_shrink_0()
                        .rounded_lg()
                        .bg(rgb(accent))
                        .text_color(rgb(0xffffff))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(format!("{:02}", index + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(track.title),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x8491a1))
                                .child(track.artist),
                        )
                        .when(track.id == 2, |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x658099))
                                    .child("Extended version"),
                            )
                        }),
                )
                .when(track.saved, |row| {
                    row.child(
                        svg()
                            .path(LucideIcons::Heart.path())
                            .size(px(18.))
                            .text_color(rgb(0xb77b7f)),
                    )
                })
                .child(
                    ReorderHandle::new(
                        &self.order,
                        track.id,
                        svg()
                            .path(LucideIcons::GripVertical.path())
                            .size(px(22.))
                            .text_color(rgb(0x8996a8)),
                    )
                    .p_3()
                    .text_color(rgb(0x8996a8)),
                );
            list = list.item(
                track.id,
                SwipeActions::new(("swipe", track.id), &track.swipe, row)
                    .rounded_xl()
                    .feedback(|swipe, _, _| {
                        let right = swipe.direction == SwipeDirection::Right;
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .px_6()
                            .when(!right, |row| row.justify_end())
                            .bg(rgb(if swipe.ready { 0x41796f } else { 0xe1eee9 }))
                            .text_color(rgb(if swipe.ready { 0xffffff } else { 0x41796f }))
                            .child(if right { "Save" } else { "Unsave" })
                    }),
            );
        }
        div().size_full().pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right).bg(rgb(0xf2f5f9)).text_color(rgb(0x25354b))
            .child(div().size_full().max_w(px(720.)).mx_auto().p_5().flex().flex_col().gap_4()
                .child(div().flex_shrink_0().text_sm().text_color(rgb(0x7589a4)).child("YOUR ROTATION"))
                .child(div().flex_shrink_0().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Set the mood."))
                .child(div().flex_shrink_0().text_sm().text_color(rgb(0x7b8c9f)).whitespace_normal()
                    .child("Hold a track to reorder, or drag its handle. Swipe right to save, left to unsave."))
                .child(div().flex_shrink_0().text_sm().text_color(rgb(0x527398)).child(self.status.clone()))
                .child(list))
    }
}
#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application()
        .with_assets(LucideAssets::new())
        .run(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(760.), px(850.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .expect("open reorderable list example");
        });
}
