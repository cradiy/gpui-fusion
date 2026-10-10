use gpui::{prelude::*, *};
use gpui_effects::{FrostedGlass, FrostedGlassAppearance, LiquidGlass, LiquidGlassAppearance};
use uic::{
    assets::{LucideAssets, LucideIcons},
    components::{
        command_palette::{
            CommandItem, CommandPalette, CommandPaletteAction, CommandPaletteEvent,
            CommandPaletteState,
        },
        modal,
    },
};

fn commands() -> Vec<CommandItem> {
    vec![
        CommandItem::new("new", "New document")
            .group("Workspace")
            .description("Start with a clean page")
            .keywords("create write")
            .icon(LucideIcons::FilePlus)
            .shortcut("Ctrl N"),
        CommandItem::new("open", "Open workspace")
            .group("Workspace")
            .description("Pick up where you left off")
            .icon(LucideIcons::FolderOpen),
        CommandItem::new("search", "Search everywhere")
            .group("Workspace")
            .description("Find a document, note or idea")
            .icon(LucideIcons::Search),
        CommandItem::new("theme", "Change appearance")
            .group("Preferences")
            .keywords("theme dark light")
            .icon(LucideIcons::Palette),
        CommandItem::new("settings", "Open settings")
            .group("Preferences")
            .icon(LucideIcons::Settings),
        CommandItem::new("sync", "Sync workspace")
            .group("Preferences")
            .description("Unavailable while offline")
            .icon(LucideIcons::RefreshCw)
            .disabled(true),
    ]
}
struct Example {
    commands: Entity<CommandPaletteState>,
    dialog: Entity<CommandPaletteState>,
    status: SharedString,
    loading: bool,
    material: usize,
    placement: usize,
    _subscriptions: Vec<Subscription>,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let inline = cx.new(|cx| CommandPaletteState::new(commands(), window, cx));
        let dialog = cx.new(|cx| CommandPaletteState::new(commands(), window, cx));
        let subscriptions = [&inline, &dialog]
            .into_iter()
            .map(|state| {
                cx.subscribe(state, |this: &mut Self, _, event, cx| {
                    if let CommandPaletteEvent::Invoked(item) = event {
                        this.status = format!("Executed: {}", item.label).into();
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            commands: inline,
            dialog,
            status: "Choose a command to get started.".into(),
            loading: false,
            material: 0,
            placement: 0,
            _subscriptions: subscriptions,
        }
    }
    fn palette(&self, state: &Entity<CommandPaletteState>) -> CommandPalette {
        let palette = CommandPalette::new(state)
            .key_binding("ctrl-p", CommandPaletteAction::Previous)
            .key_binding("ctrl-n", CommandPaletteAction::Next);
        let palette = match self.placement {
            1 => palette.placement(modal::ModalPlacement::Center),
            2 => palette.placement(modal::ModalPlacement::Bottom {
                avoid_safe_area: true,
                drag_to_dismiss: false,
            }),
            _ => palette,
        };
        match self.material {
            1 => palette.surface(|content, _, _| {
                FrostedGlass::with_appearance(
                    FrostedGlassAppearance::light()
                        .tint(rgba(0xf7faffb0).into())
                        .blur_radius(px(14.)),
                )
                .child(content)
            }),
            2 => palette.surface(|content, _, _| {
                LiquidGlass::with_appearance(
                    LiquidGlassAppearance::regular()
                        .tint(rgba(0xf7faffa8).into())
                        .blur_radius(px(16.))
                        .clarity(0.08),
                )
                .child(content)
            }),
            _ => palette,
        }
    }
}
impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().effective();
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let heading = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x8190a5))
                    .child("ONE PLACE TO GET THINGS DONE"),
            )
            .child(
                div()
                    .text_3xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("What’s next?"),
            )
            .child(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .text_color(rgb(0x8190a5))
                    .child("Search commands, use the arrow keys, then press Enter."),
            );
        let buttons = div()
            .flex()
            .flex_wrap()
            .gap_3()
            .child(
                div()
                    .id("open-palette")
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0x5374c6))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.palette(&this.dialog)
                            .shadow_lg()
                            .show(window, cx);
                    }))
                    .child("Open as dialog"),
            )
            .child(
                div()
                    .id("toggle-loading")
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0xe9eef8))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.loading = !this.loading;
                        this.commands
                            .update(cx, |state, cx| state.set_loading(this.loading, cx));
                        cx.notify();
                    }))
                    .child(if self.loading {
                        "Show commands"
                    } else {
                        "Loading preview"
                    }),
            )
            .child(
                div()
                    .id("material")
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0xe9eef8))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.material = (this.material + 1) % 3;
                        cx.notify();
                    }))
                    .child(match self.material {
                        1 => "Surface: Frosted",
                        2 => "Surface: Liquid",
                        _ => "Surface: Plain",
                    }),
            )
            .child(
                div()
                    .id("placement")
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0xe9eef8))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.placement = (this.placement + 1) % 3;
                        cx.notify();
                    }))
                    .child(match self.placement {
                        1 => "Dialog: Center",
                        2 => "Dialog: Bottom",
                        _ => "Dialog: Top",
                    }),
            );
        let palette = self.palette(&self.commands).w_full();
        let content = div()
            .mx_auto()
            .w_full()
            .max_w(px(640.))
            .flex()
            .flex_col()
            .gap_5()
            .child(heading)
            .child(buttons)
            .child(palette)
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x61728a))
                    .child(self.status.clone()),
            );
        div()
            .relative()
            .size_full()
            .bg(rgb(0xf3f5f9))
            .text_color(rgb(0x26364d))
            .pt(safe.top)
            .pb(safe.bottom)
            .pl(safe.left)
            .pr(safe.right)
            .child(
                div()
                    .absolute()
                    .left(px(35.))
                    .top(px(300.))
                    .size(px(220.))
                    .rounded_full()
                    .bg(rgb(0xb7d1f8)),
            )
            .child(
                div()
                    .absolute()
                    .right(px(25.))
                    .top(px(450.))
                    .size(px(170.))
                    .rounded_full()
                    .bg(rgb(0xe9c8db)),
            )
            .child(
                div()
                    .id("page")
                    .size_full()
                    .overflow_y_scroll()
                    .p_5()
                    .child(content),
            )
            .child(modal::layer(cx))
    }
}
#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application()
        .with_assets(LucideAssets::new())
        .run(|cx| {
            uic::init(cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(720.), px(780.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .expect("open command palette example");
        });
}
