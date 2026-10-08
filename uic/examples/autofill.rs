use gpui::{AutofillHint, Context, Entity, Render, Window, div, prelude::*, px, rgb};
use uic::components::input::{Input, InputEvent, TextInput};

struct AutofillExample {
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    status: String,
    completed: bool,
}

impl AutofillExample {
    fn new(cx: &mut Context<Self>) -> Self {
        let username = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("Username or email")
                .autofill("login-username", AutofillHint::Username)
        });
        let password = cx.new(|cx| {
            TextInput::new(cx)
                .password()
                .placeholder("Password")
                .autofill("login-password", AutofillHint::Password)
        });
        for input in [&username, &password] {
            cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change(_)) {
                    this.status = "Fields updated. No credentials are sent.".into();
                    cx.notify();
                }
            })
            .detach();
        }
        Self {
            username,
            password,
            status: "Choose an account from your password manager.".into(),
            completed: false,
        }
    }
}

impl Render for AutofillExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().items_center().justify_center().p_6()
            .bg(rgb(0xf4f6f8)).text_color(rgb(0x172033))
            .child(div().w_full().max_w(px(420.)).p_8().rounded(px(20.)).bg(rgb(0xffffff))
                .shadow_lg().flex().flex_col().gap_4()
                .child(div().text_2xl().child("Welcome back"))
                .child(div().text_sm().text_color(rgb(0x64748b)).child("System autofill · GPUI appearance"))
                .when(!self.completed, |card| card
                .child(Input::new(&self.username))
                .child(Input::new(&self.password))
                .child(div().id("finish-autofill").p_3().rounded(px(8.)).bg(rgb(0x315be8))
                    .text_color(rgb(0xffffff)).cursor_pointer().child("Finish demo form")
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.username.read(cx).value().is_empty() || this.password.read(cx).value().is_empty() {
                            this.status = "Enter a test username and password first.".into();
                            cx.notify();
                            return;
                        }
                        this.status = match window.commit_autofill() {
                            Ok(()) => {
                                this.completed = true;
                                "Form completed. Your system decides whether to offer saving.".into()
                            },
                            Err(error) => error.to_string(),
                        };
                        cx.notify();
                    }))))
                .when(self.completed, |card| card.child(
                    div().id("restart-autofill").p_3().rounded(px(8.)).bg(rgb(0x315be8))
                        .text_color(rgb(0xffffff)).cursor_pointer().child("Try again")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.username.update(cx, |input, cx| input.clear(cx));
                            this.password.update(cx, |input, cx| input.clear(cx));
                            this.completed = false;
                            this.status = "Choose an account from your password manager.".into();
                            cx.notify();
                        }))
                ))
                .child(div().text_sm().whitespace_normal().text_color(rgb(0x64748b)).child(
                    if window.supports_autofill() { self.status.clone() } else {
                        "This platform has no system autofill adapter. You can still enter or paste credentials.".into()
                    }
                )))
    }
}

#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application().run(|cx| {
        uic::init(cx);
        cx.open_window(Default::default(), |_, cx| cx.new(AutofillExample::new))
            .expect("open autofill example");
    });
}
