use super::Counter;
use gpui::Context;
use gpui::gpui_io::{CreateOptions, LocationBookmark, SystemLocation};

pub enum Action {
    Choose,
    Write,
    Restore,
    Release,
}

impl Counter {
    pub fn directory_action(&mut self, action: Action, cx: &mut Context<Self>) {
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
        let picker = matches!(action, Action::Choose).then(|| cx.prompt_for_directory());
        let selected = self.directory.clone();
        let contents = self.file_text.read(cx).value().to_string().into_bytes();
        self.file_pending = true;
        self.file_status = "Updating directory access...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<_> = async {
                let location = io.location(SystemLocation::AppData).await?;
                let storage = location.file("directory-bookmark.json")?;
                if matches!(action, Action::Write) {
                    let directory = selected.ok_or_else(|| anyhow::anyhow!("Choose or restore a directory first"))?;
                    let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis();
                    let file = directory.create_file(format!("GPUI/Samples/note-{id}.txt"), CreateOptions { mime_type: Some("text/plain".into()) }).await?;
                    file.write(contents).await?;
                    return Ok((format!("Saved GPUI/Samples/{}", file.name()), Some(directory)));
                }
                let previous: Option<LocationBookmark> = match storage.read_limited(128 * 1024).await {
                    Ok(bytes) => serde_json::from_slice(&bytes)?,
                    Err(error) if error.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => None,
                    Err(error) => return Err(error),
                };
                match action {
                    Action::Choose => {
                        let Some(directory) = picker.unwrap().await?? else { return Ok(("Directory selection cancelled.".into(), selected)); };
                        anyhow::ensure!(previous.is_none(), "Forget the saved directory before choosing another");
                        if let Err(error) = location.create_file("directory-bookmark.json", CreateOptions::default()).await
                            && !error.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::AlreadyExists) { return Err(error); }
                        storage.write(b"null".to_vec()).await?;
                        let bookmark = directory.persist().await?;
                        if let Err(error) = storage.write(serde_json::to_vec(&Some(&bookmark))?).await {
                            let _ = io.release_location(&bookmark).await;
                            return Err(error);
                        }
                        Ok(("Directory access retained. Restart the app and choose Restore directory.".into(), Some(directory)))
                    }
                    Action::Restore => {
                        let bookmark = previous.ok_or_else(|| anyhow::anyhow!("No saved directory"))?;
                        Ok(("Directory restored.".into(), Some(io.restore_location(&bookmark).await?)))
                    }
                    Action::Release => {
                        if let Some(bookmark) = previous { io.release_location(&bookmark).await?; }
                        storage.write(b"null".to_vec()).await?;
                        Ok(("Directory access released. Contents were not deleted.".into(), None))
                    }
                    Action::Write => unreachable!(),
                }
            }.await;
            let _ = this.update(cx, |this, cx| {
                this.file_pending = false;
                match result {
                    Ok((status, directory)) => { this.file_status = status; this.directory = directory; }
                    Err(error) => this.file_status = format!("Directory: {error:#}"),
                }
                cx.notify();
            });
        }).detach();
    }
}
