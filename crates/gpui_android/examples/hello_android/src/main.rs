use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};
use uic::components::input::{Input, InputActionEvent, InputEvent, InputMode, TextInput};

const INPUT_ACTIONS: [TextInputAction; 6] = [
    TextInputAction::Next,
    TextInputAction::Search,
    TextInputAction::Go,
    TextInputAction::Send,
    TextInputAction::Previous,
    TextInputAction::Done,
];

const KEYBOARDS: [(&str, TextInputPurpose); 6] = [
    ("Email", TextInputPurpose::Email),
    ("URL", TextInputPurpose::Url),
    ("Phone", TextInputPurpose::Phone),
    (
        "Digits",
        TextInputPurpose::Number {
            decimal: false,
            signed: false,
        },
    ),
    (
        "Signed decimal",
        TextInputPurpose::Number {
            decimal: true,
            signed: true,
        },
    ),
    ("Text", TextInputPurpose::Text),
];

#[gpui_platform::main]
fn main() {
    let application = gpui_platform::application();
    application.on_receive_share(|result, cx| {
        let state = cx.global_mut::<SharedContent>();
        state.count += 1;
        state.preview.clear();
        match result {
            Ok(share) => {
                state.status = format!(
                    "Share {}: {} · {} file(s)",
                    state.count,
                    share.mime_type.as_deref().unwrap_or("unspecified type"),
                    share.files.len()
                );
                state.content = Some(share);
            }
            Err(error) => {
                state.status = format!("Share {}: {error}", state.count);
                state.content = None;
            }
        }
        cx.refresh_windows();
    });
    let link_context = Rc::new(RefCell::new(None::<AsyncApp>));
    let receiver = link_context.clone();
    application.on_open_urls(move |urls| {
        if let Some(cx) = receiver.borrow().as_ref() {
            cx.update(|cx| {
                let links = cx.global_mut::<OpenedLinks>();
                links.count += urls.len();
                if let Some(url) = urls.last() {
                    links.last = url.clone();
                }
                cx.refresh_windows();
            });
        }
    });
    application.run(move |cx| {
        cx.set_global(SharedContent::default());
        cx.set_global(OpenedLinks::default());
        *link_context.borrow_mut() = Some(cx.to_async());
        uic::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| Counter {
                details: false,
                count: 0,
                scroll: ScrollHandle::new(),
                clipboard_status: "Copy the counter or paste text from another app.".into(),
                file_status: "Choose a file to read its contents.".into(),
                file_pending: false,
                credential_pending: false,
                credential_status: "Check encrypted storage with disposable sample data.".into(),
                document: None,
                file_text: cx.new(|cx| {
                    TextInput::new(cx)
                        .multiline()
                        .initial_value("Hello from GPUI — 你好！\n")
                }),
                permission_status: "Microphone permission has not been requested.".into(),
                #[cfg(target_os = "android")]
                permissions: gpui_android::current_platform().permissions(),
                title: cx.new(|cx| TextInput::new(cx).placeholder("Name")),
                text: cx.new(|cx| TextInput::new(cx).multiline().placeholder("Message")),
                password: cx.new(|cx| TextInput::new(cx).password().placeholder("Password")),
                password_visible: false,
                submissions: 0,
                keyboard: 0,
                input_action: 0,
                action_status: "No keyboard action yet.".into(),
                reply: cx.new(|cx| {
                    TextInput::new(cx)
                        .multiline()
                        .input_action(TextInputAction::Send)
                        .placeholder("Reply")
                }),
                keyboard_input: cx.new(|cx| {
                    TextInput::new(cx)
                        .input_purpose(KEYBOARDS[0].1)
                        .input_action(INPUT_ACTIONS[0])
                        .placeholder("Try a keyboard layout")
                }),
            });
            view.update(cx, |this, cx| {
                cx.subscribe(&this.title, |this, _, event, cx| {
                    if matches!(event, InputEvent::Submit(_)) {
                        this.submissions += 1;
                        cx.notify();
                    }
                })
                .detach();
                cx.subscribe_in(
                    &this.keyboard_input,
                    window,
                    |this, _, event: &InputActionEvent, window, cx| {
                        this.action_status = format!("Action: {:?}", event.action);
                        match event.action {
                            TextInputAction::Next => {
                                window.focus(&this.reply.focus_handle(cx), cx);
                                window.show_soft_keyboard();
                            }
                            TextInputAction::Previous => {
                                window.focus(&this.title.focus_handle(cx), cx);
                                window.show_soft_keyboard();
                            }
                            _ => {}
                        }
                        cx.notify();
                    },
                )
                .detach();
                cx.subscribe(&this.reply, |this, _, event: &InputActionEvent, cx| {
                    if event.action == TextInputAction::Send {
                        this.action_status =
                            format!("Send: {} characters", event.text.chars().count());
                        this.reply.update(cx, |input, cx| input.clear(cx));
                        cx.notify();
                    }
                })
                .detach();
            });
            window.on_system_back(
                cx,
                window.handler_for(&view, |this, window, cx| {
                    this.details = false;
                    window.set_back_enabled(false);
                    cx.notify();
                }),
            );
            view
        })
        .expect("failed to open the GPUI window");
    });
}

#[derive(Default)]
struct OpenedLinks {
    count: usize,
    last: String,
}
impl Global for OpenedLinks {}

#[derive(Default)]
struct SharedContent {
    count: usize,
    status: String,
    content: Option<ReceivedShare>,
    preview: String,
}
impl Global for SharedContent {}

struct Counter {
    details: bool,
    count: usize,
    scroll: ScrollHandle,
    clipboard_status: String,
    file_status: String,
    file_pending: bool,
    credential_pending: bool,
    credential_status: String,
    document: Option<SelectedFile>,
    file_text: Entity<TextInput>,
    permission_status: String,
    #[cfg(target_os = "android")]
    permissions: gpui_android::AndroidPermissions,
    text: Entity<TextInput>,
    title: Entity<TextInput>,
    password: Entity<TextInput>,
    password_visible: bool,
    submissions: usize,
    keyboard: usize,
    keyboard_input: Entity<TextInput>,
    input_action: usize,
    action_status: String,
    reply: Entity<TextInput>,
}

#[derive(Clone, Copy)]
enum BookmarkAction {
    Remember,
    Restore,
    Release,
}

enum ShareAction {
    Text,
    Document,
    ChooseFiles,
}

fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .p_4()
        .rounded_lg()
        .bg(rgb(0x375c91))
        .text_color(rgb(0xe7edf7))
        .child(label)
}

impl Counter {
    fn share_content(&mut self, action: ShareAction, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        let mut options = ShareOptions::default();
        let selection = match action {
            ShareAction::Text => {
                options.text = Some(self.file_text.read(cx).value().to_string());
                None
            }
            ShareAction::Document => {
                let Some(file) = self.document.clone() else {
                    self.file_status = "Open or save a document first.".into();
                    cx.notify();
                    return;
                };
                options.files.push(file);
                None
            }
            ShareAction::ChooseFiles => Some(cx.prompt_for_files(FilePromptOptions {
                multiple: true,
                ..Default::default()
            })),
        };
        self.file_pending = true;
        self.file_status = "Preparing share...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<()>> = async {
                if let Some(selection) = selection {
                    let Some(files) = selection.await?? else {
                        return Ok(None);
                    };
                    options.files = files;
                }
                let request = cx.update(|cx| cx.share(options));
                request.await?;
                Ok(Some(()))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status = match result {
                    Ok(Some(())) => "Share sheet requested.".into(),
                    Ok(None) => "Selection cancelled.".into(),
                    Err(error) => format!("Share: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn open_document_with_system(&mut self, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        let Some(document) = self.document.as_ref() else {
            self.file_status = "Open or save a document first.".into();
            cx.notify();
            return;
        };
        let request = cx.open_file_with_system(document);
        self.file_pending = true;
        self.file_status = "Opening file...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status = match result {
                    Ok(()) => "Open request sent to the system.".into(),
                    Err(error) => format!("Open with system: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn file_bookmark(&mut self, action: BookmarkAction, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        let io = match cx.file_system("dev.gpui.example") {
            Ok(io) => io,
            Err(error) => {
                self.file_status = error.to_string();
                cx.notify();
                return;
            }
        };
        let document = self.document.clone();
        self.file_pending = true;
        self.file_status = "Updating file access...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<_> = async {
                use gpui::gpui_io::{CreateOptions, FileBookmark, SystemLocation};
                let location = io.location(SystemLocation::AppData).await?;
                let storage = location.file("document-bookmark.json")?;
                let previous: Option<FileBookmark> = match storage.read_limited(128 * 1024).await {
                    Ok(bytes) => serde_json::from_slice(&bytes)?,
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        None
                    }
                    Err(error) => return Err(error),
                };
                match action {
                    BookmarkAction::Remember => {
                        anyhow::ensure!(
                            previous.is_none(),
                            "Forget the saved file before remembering another"
                        );
                        let document = document.ok_or_else(|| {
                            anyhow::anyhow!("Open a document or use Save as first")
                        })?;
                        // Ensure private storage exists before retaining a system grant.
                        if let Err(error) = location
                            .create_file("document-bookmark.json", CreateOptions::default())
                            .await
                            && !error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                                error.kind() == std::io::ErrorKind::AlreadyExists
                            })
                        {
                            return Err(error);
                        }
                        storage.write(b"null".to_vec()).await?;
                        let bookmark = document.persist().await?;
                        storage.write(serde_json::to_vec(&Some(bookmark))?).await?;
                        Ok((
                            "File access retained. Restart the app, then choose Restore file."
                                .into(),
                            None,
                        ))
                    }
                    BookmarkAction::Restore => {
                        let bookmark =
                            previous.ok_or_else(|| anyhow::anyhow!("No saved file bookmark"))?;
                        let file = io.restore_file(&bookmark).await?;
                        let text = String::from_utf8(file.read_limited(4 * 1024 * 1024).await?)?;
                        Ok((format!("Restored {}", file.name()), Some((file, text))))
                    }
                    BookmarkAction::Release => {
                        let bookmark =
                            previous.ok_or_else(|| anyhow::anyhow!("No saved file bookmark"))?;
                        io.release_file(&bookmark).await?;
                        storage.write(b"null".to_vec()).await?;
                        Ok((
                            "Persistent access released. The file was not deleted.".into(),
                            None,
                        ))
                    }
                }
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                match result {
                    Ok((status, restored)) => {
                        this.file_status = status;
                        if let Some((file, text)) = restored {
                            this.file_text
                                .update(cx, |input, cx| input.set_value(text, cx));
                            this.document = Some(file);
                        }
                    }
                    Err(error) => this.file_status = format!("File access: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn check_credentials(&mut self, cx: &mut Context<Self>) {
        if self.credential_pending {
            return;
        }
        self.credential_pending = true;
        self.credential_status = "Checking credential storage...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let url = format!(
                "gpui-example://credentials/{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let result: anyhow::Result<()> = async {
                anyhow::ensure!(
                    cx.update(|cx| cx.read_credentials(&url)).await?.is_none(),
                    "Expected an empty sample entry"
                );
                for (username, password) in [("示例用户", &[0, 255, 128, 42][..]), ("", &[][..])]
                {
                    cx.update(|cx| cx.write_credentials(&url, username, password))
                        .await?;
                    let stored = cx.update(|cx| cx.read_credentials(&url)).await?;
                    anyhow::ensure!(
                        stored == Some((username.to_owned(), password.to_vec())),
                        "Credential round trip failed"
                    );
                }
                Ok(())
            }
            .await;
            let cleanup = cx.update(|cx| cx.delete_credentials(&url)).await;
            let result = result.and(cleanup);
            let result = match result {
                Ok(()) => cx
                    .update(|cx| cx.read_credentials(&url))
                    .await
                    .and_then(|value| {
                        anyhow::ensure!(value.is_none(), "Sample entry was not removed");
                        Ok(())
                    }),
                error => error,
            };
            let _ = this.update(cx, |this, cx| {
                this.credential_pending = false;
                this.credential_status = match result {
                    Ok(()) => "Credential write, read, overwrite and delete passed.".into(),
                    Err(error) => format!("Credential storage: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn save_in_location(
        &mut self,
        location: gpui::gpui_io::SystemLocation,
        cx: &mut Context<Self>,
    ) {
        if self.file_pending {
            return;
        }
        let io = match cx.file_system("dev.gpui.example") {
            Ok(io) => io,
            Err(error) => {
                self.file_status = format!("Storage: {error}");
                cx.notify();
                return;
            }
        };
        let contents = self.file_text.read(cx).value().as_bytes().to_vec();
        self.file_pending = true;
        self.file_status = "Writing file...".into();
        cx.spawn(async move |this, cx| {
            let result = async {
                let target = io.location(location).await?;
                let relative_path = format!(
                    "GPUI/Samples/gpui-note-{}.txt",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_millis()
                );
                let file = target
                    .create_file(
                        relative_path,
                        gpui::gpui_io::CreateOptions {
                            mime_type: Some("text/plain".into()),
                        },
                    )
                    .await?;
                let mut writer = file
                    .open_write(gpui::gpui_io::WriteOptions::truncate())
                    .await?;
                for chunk in contents.chunks(4096) {
                    writer.write_all(chunk).await?;
                }
                writer.close().await?;
                anyhow::ensure!(file.read().await? == contents, "file read-back differs");
                Ok::<_, anyhow::Error>(file)
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status = match result {
                    Ok(file) => {
                        let status = format!("Saved to {location:?}/GPUI/Samples: {}", file.name());
                        this.document = Some(file);
                        status
                    }
                    Err(error) => format!("Storage: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose_files(&mut self, multiple: bool, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        self.file_pending = true;
        self.file_status = "Choosing files...".into();
        let selection = cx.prompt_for_files(FilePromptOptions {
            multiple,
            ..Default::default()
        });
        cx.spawn(async move |this, cx| {
            let result = async {
                let Some(files) = selection.await?? else {
                    return Ok::<_, anyhow::Error>("Selection cancelled.".to_string());
                };
                let _ = this.update(cx, |this, cx| {
                    this.file_status = "Reading files...".into();
                    cx.notify();
                });
                let mut summaries = Vec::new();
                for file in files {
                    let bytes = file.read().await?;
                    summaries.push(format!("{} · {} bytes", file.name(), bytes.len()));
                }
                Ok(summaries.join("\n"))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status =
                    result.unwrap_or_else(|error| format!("File selection: {error}"));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_document(&mut self, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        self.file_pending = true;
        self.file_status = "Opening a text document...".into();
        let selection = cx.prompt_for_files(FilePromptOptions {
            multiple: false,
            writable: true,
        });
        cx.spawn(async move |this, cx| {
            let result = async {
                let Some(files) = selection.await?? else {
                    return Ok::<_, anyhow::Error>(None);
                };
                let file = files
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No document selected"))?;
                let text = String::from_utf8(file.read_limited(4 * 1024 * 1024).await?)?;
                Ok(Some((file, text)))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status = match result {
                    Ok(Some((file, text))) => {
                        let status = format!("Editing {}", file.name());
                        this.file_text
                            .update(cx, |input, cx| input.set_value(text, cx));
                        this.document = Some(file);
                        status
                    }
                    Ok(None) => "Selection cancelled.".into(),
                    Err(error) => format!("Open: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save_document(&mut self, save_as: bool, cx: &mut Context<Self>) {
        if self.file_pending {
            return;
        }
        if !save_as && self.document.is_none() {
            self.file_status = "Use Save as or open a document for editing first.".into();
            cx.notify();
            return;
        }
        let selection = save_as.then(|| {
            cx.prompt_for_file_save(FileSaveOptions {
                suggested_name: "gpui-note.txt".into(),
                mime_type: "text/plain".into(),
                ..Default::default()
            })
        });
        let document = self.document.clone();
        let contents = self.file_text.read(cx).value().as_bytes().to_vec();
        let count = contents.len();
        self.file_pending = true;
        self.file_status = "Saving...".into();
        cx.spawn(async move |this, cx| {
            let result = async {
                let file = match selection {
                    Some(selection) => selection.await??,
                    None => document,
                };
                let Some(file) = file else {
                    return Ok::<_, anyhow::Error>(None);
                };
                file.write(contents).await?;
                Ok(Some(file))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                this.file_status = match result {
                    Ok(Some(file)) => {
                        let status = format!("Saved {} · {count} bytes", file.name());
                        this.document = Some(file);
                        status
                    }
                    Ok(None) => "Save cancelled.".into(),
                    Err(error) => format!("Save: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn request_microphone(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "android")]
        {
            let permissions = self.permissions.clone();
            self.permission_status = "Waiting for Android permission...".into();
            cx.spawn(async move |this, cx| {
                let result = permissions.request("android.permission.RECORD_AUDIO").await;
                let _ = this.update(cx, |this, cx| {
                    this.permission_status = match result {
                        Ok(status) => format!("Microphone: {status:?}"),
                        Err(error) => format!("Permission request: {error}"),
                    };
                    cx.notify();
                });
            })
            .detach();
        }
        #[cfg(not(target_os = "android"))]
        {
            self.permission_status = "Permission requests are available on Android.".into();
        }
        cx.notify();
    }
}

impl Render for Counter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let (background, foreground, muted, surface) = if dark {
            (0x101923, 0xe7edf7, 0xa0b1c6, 0x1e2d40)
        } else {
            (0xf4f7fb, 0x172033, 0x52647a, 0xe3eaf3)
        };
        window.set_back_enabled(self.details);
        if self.details {
            return div()
                .id("details-page")
                .size_full()
                .overflow_y_scroll()
                .bg(rgb(background))
                .text_color(rgb(foreground))
                .font_family("IBM Plex Sans")
                .p_6()
                .flex()
                .flex_col()
                .gap_5()
                .child(div().text_3xl().child("Details"))
                .child("System Back returns to the main page. With the keyboard open, Back hides it first.")
                .child(Input::new(&self.title).text_color(rgb(0x172033)))
                .child(div().text_sm().child(format!("Keyboard: {} · Action: {:?}", KEYBOARDS[self.keyboard].0, INPUT_ACTIONS[self.input_action])))
                .child(Input::new(&self.keyboard_input).text_color(rgb(0x172033)))
                .child(button("keyboard-purpose", "Change keyboard").on_click(cx.listener(|this, _, window, cx| {
                    this.keyboard = (this.keyboard + 1) % KEYBOARDS.len();
                    this.keyboard_input.update(cx, |input, cx| {
                        input.set_input_purpose(KEYBOARDS[this.keyboard].1, cx);
                    });
                    window.focus(&this.keyboard_input.focus_handle(cx), cx);
                    window.show_soft_keyboard();
                    cx.notify();
                })))
                .child(button("keyboard-action", "Change action").on_click(cx.listener(|this, _, window, cx| {
                    this.input_action = (this.input_action + 1) % INPUT_ACTIONS.len();
                    this.keyboard_input.update(cx, |input, cx| input.set_input_action(Some(INPUT_ACTIONS[this.input_action]), cx));
                    window.focus(&this.keyboard_input.focus_handle(cx), cx);
                    window.show_soft_keyboard();
                    cx.notify();
                })))
                .child(Input::new(&self.reply).rows(2).text_color(rgb(0x172033)))
                .child(div().text_sm().child(self.action_status.clone()))
                .child(div().flex().flex_wrap().gap_3()
                    .child(button("edit-name", "Edit name").on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.title.focus_handle(cx), cx);
                        window.show_soft_keyboard();
                    })))
                    .child(button("hide-keyboard", "Hide keyboard").on_click(|_, window, _| {
                        window.hide_soft_keyboard();
                    })))
                .child(button("back", "Back to main page").on_click(cx.listener(|this, _, window, cx| {
                    this.details = false;
                    window.set_back_enabled(false);
                    cx.notify();
                })))
                .into_any_element();
        }
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .font_family("IBM Plex Sans")
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_3xl().child("GPUI on Android"))
            .when(cx.global::<SharedContent>().count > 0, |page| {
                let shared = cx.global::<SharedContent>();
                let content = shared.content.clone();
                page.child(
                    div()
                        .p_5()
                        .rounded_xl()
                        .bg(rgb(surface))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child("Received share")
                        .child(
                            div()
                                .text_sm()
                                .whitespace_normal()
                                .child(shared.status.clone()),
                        )
                        .child(
                            div().whitespace_normal().child(
                                content
                                    .as_ref()
                                    .and_then(|share| share.text.clone())
                                    .unwrap_or_default(),
                            ),
                        )
                        .children(content.iter().flat_map(|share| &share.files).map(|file| {
                            div()
                                .text_sm()
                                .whitespace_normal()
                                .child(file.name().to_owned())
                        }))
                        .when(
                            content
                                .as_ref()
                                .is_some_and(|share| !share.files.is_empty()),
                            |card| {
                                card.child(
                                    button("read-shared-file", "Read first file (up to 4 KiB)")
                                        .on_click(|_, _, cx| {
                                            let shared = cx.global::<SharedContent>();
                                            let count = shared.count;
                                            let Some(file) = shared
                                                .content
                                                .as_ref()
                                                .and_then(|share| share.files.first())
                                                .cloned()
                                            else {
                                                return;
                                            };
                                            cx.spawn(async move |cx| {
                                                let result = file.read_limited(4096).await;
                                                cx.update(|cx| {
                                                    let shared = cx.global_mut::<SharedContent>();
                                                    if shared.count == count {
                                                        shared.preview = match result {
                                                            Ok(bytes) => {
                                                                String::from_utf8_lossy(&bytes)
                                                                    .into_owned()
                                                            }
                                                            Err(error) => format!("Read: {error}"),
                                                        };
                                                        cx.refresh_windows();
                                                    }
                                                });
                                            })
                                            .detach();
                                        }),
                                )
                            },
                        )
                        .child(
                            div()
                                .text_sm()
                                .whitespace_normal()
                                .child(shared.preview.clone()),
                        ),
                )
            })
            .child(
                button("microphone", "Request microphone permission")
                    .on_click(cx.listener(Self::request_microphone)),
            )
            .child(div().text_sm().child(self.permission_status.clone()))
            .child(
                button("details", "Open details").on_click(cx.listener(|this, _, _, cx| {
                    this.details = true;
                    cx.notify();
                })),
            )
            .child(div().text_color(rgb(muted)).child(if dark {
                "Android View · Dark appearance"
            } else {
                "Android View · Light appearance"
            }))
            .child(Input::new(&self.title).text_color(rgb(0x172033)))
            .child(
                div()
                    .text_sm()
                    .child(format!("Name submissions: {}", self.submissions)),
            )
            .child(Input::new(&self.text).rows(2).text_color(rgb(0x172033)))
            .child(Input::new(&self.password).text_color(rgb(0x172033)))
            .child(
                button("password-mode", "Show / hide password").on_click(cx.listener(
                    |this, _, window, cx| {
                        this.password_visible = !this.password_visible;
                        this.password.update(cx, |input, cx| {
                            input.set_mode(if this.password_visible {
                                InputMode::Text
                            } else {
                                InputMode::Password
                            });
                            cx.notify();
                        });
                        window.focus(&this.password.focus_handle(cx), cx);
                        window.show_soft_keyboard();
                    },
                )),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(surface))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_2xl().child(format!("{} taps", self.count)))
                    .child(button("increment", "Tap to count").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.count += 1;
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(muted))
                    .child("Rotate or switch apps. Your count stays here."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_5()
                    .rounded_xl()
                    .bg(rgb(surface))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_xl().child("Clipboard & links"))
                    .child(
                        button("check-credentials", "Check credential storage")
                            .on_click(cx.listener(|this, _, _, cx| this.check_credentials(cx))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .whitespace_normal()
                            .child(self.credential_status.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(button("copy", "Copy count").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let task =
                                        cx.write_to_clipboard_async(ClipboardItem::new_string(
                                            format!("GPUI: {} taps", this.count),
                                        ));
                                    cx.spawn(async move |this, cx| {
                                        let result = task.await;
                                        let _ = this.update(cx, |this, cx| {
                                            this.clipboard_status = result
                                                .map(|_| "Counter copied.".into())
                                                .unwrap_or_else(|error| error.to_string());
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                },
                            )))
                            .child(button("paste", "Paste text").on_click(cx.listener(
                                |_, _, _, cx| {
                                    let task = cx.read_from_clipboard_async();
                                    cx.spawn(async move |this, cx| {
                                        let result = task.await;
                                        let _ = this.update(cx, |this, cx| {
                                            this.clipboard_status = match result {
                                                Ok(Some(item)) => format!(
                                                    "Pasted: {}",
                                                    item.text().unwrap_or_default()
                                                ),
                                                Ok(None) => "No text available to paste.".into(),
                                                Err(error) => error.to_string(),
                                            };
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                },
                            ))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .whitespace_normal()
                            .text_color(rgb(muted))
                            .child(self.clipboard_status.clone()),
                    )
                    .child(button("open-link", "Open website").on_click(|_, _, cx| {
                        cx.open_url("https://www.rust-lang.org/");
                    }))
                    .child(div().text_sm().whitespace_normal().child(format!(
                        "Opened links: {} · {}",
                        cx.global::<OpenedLinks>().count,
                        cx.global::<OpenedLinks>().last,
                    ))),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_5()
                    .rounded_xl()
                    .bg(rgb(surface))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_xl().child("Files"))
                    .child(
                        Input::new(&self.file_text)
                            .rows(3)
                            .text_color(rgb(0x172033)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                button("open-document", "Open for editing")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_document(cx))),
                            )
                            .child(button("save-as", "Save as").on_click(
                                cx.listener(|this, _, _, cx| this.save_document(true, cx)),
                            ))
                            .child(button("save-document", "Save").on_click(
                                cx.listener(|this, _, _, cx| this.save_document(false, cx)),
                            ))
                            .child(
                                button("remember-file", "Remember file").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.file_bookmark(BookmarkAction::Remember, cx)
                                    },
                                )),
                            )
                            .child(button("restore-file", "Restore file").on_click(cx.listener(
                                |this, _, _, cx| this.file_bookmark(BookmarkAction::Restore, cx),
                            )))
                            .child(button("forget-file", "Forget file").on_click(cx.listener(
                                |this, _, _, cx| this.file_bookmark(BookmarkAction::Release, cx),
                            )))
                            .child(button("open-with-system", "Open with system").on_click(
                                cx.listener(|this, _, _, cx| this.open_document_with_system(cx)),
                            ))
                            .child(button("share-text", "Share text").on_click(cx.listener(
                                |this, _, _, cx| this.share_content(ShareAction::Text, cx),
                            )))
                            .child(button("share-file", "Share file").on_click(cx.listener(
                                |this, _, _, cx| this.share_content(ShareAction::Document, cx),
                            )))
                            .child(button("share-files", "Share files").on_click(cx.listener(
                                |this, _, _, cx| this.share_content(ShareAction::ChooseFiles, cx),
                            )))
                            .child(
                                button("save-app-data", "Save app data").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.save_in_location(
                                            gpui::gpui_io::SystemLocation::AppData,
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(button("save-downloads", "Save to Downloads").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.save_in_location(
                                        gpui::gpui_io::SystemLocation::Downloads,
                                        cx,
                                    )
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(button("choose-file", "Choose file").on_click(
                                cx.listener(|this, _, _, cx| this.choose_files(false, cx)),
                            ))
                            .child(button("choose-files", "Choose files").on_click(
                                cx.listener(|this, _, _, cx| this.choose_files(true, cx)),
                            )),
                    )
                    .child(
                        div()
                            .text_sm()
                            .whitespace_normal()
                            .text_color(rgb(muted))
                            .child(self.file_status.clone()),
                    ),
            )
            .child(div().text_xl().child("Swipe to explore"))
            .children((1usize..=20).map(|index| {
                div()
                    .id(("row", index))
                    .flex_shrink_0()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(surface))
                    .child(format!("Item {index:02}"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }
}
