use crate::{
    theme::StyleError,
    theme::catalog::{ThemeRepository, ThemeSource},
    theme::runtime::ThemeRuntime,
};
use gpui::{App, AppContext, Context, Entity};

/// Retain this entity to own a theme source and its optional file subscription.
pub struct ThemeSession {
    repository: ThemeRepository,
    source: ThemeSource,
    runtime: Entity<ThemeRuntime>,
    error: Option<StyleError>,
    #[cfg(feature = "theme-watch")]
    watch: Option<notify::RecommendedWatcher>,
    #[cfg(feature = "theme-watch")]
    task: Option<gpui::Task<()>>,
}

impl ThemeSession {
    /// Loads a selection without enabling automatic file updates.
    pub fn install(
        repository: ThemeRepository,
        source: ThemeSource,
        cx: &mut App,
    ) -> Result<Entity<Self>, StyleError> {
        let config = repository.load(&source)?;
        let runtime = ThemeRuntime::install_config(config, cx)?;
        Ok(cx.new(|_| Self {
            repository,
            source,
            runtime,
            error: None,
            #[cfg(feature = "theme-watch")]
            watch: None,
            #[cfg(feature = "theme-watch")]
            task: None,
        }))
    }

    pub fn runtime(&self) -> &Entity<ThemeRuntime> {
        &self.runtime
    }
    pub fn source(&self) -> &ThemeSource {
        &self.source
    }
    pub fn error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }

    /// Changes the selection only after its configuration has been validated.
    pub fn set_source(
        &mut self,
        source: ThemeSource,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let config = self.repository.load(&source)?;
        #[cfg(feature = "theme-watch")]
        let prepared = if self.watch.is_some() {
            Some(self.prepare_watch(&source)?)
        } else {
            None
        };
        self.runtime
            .update(cx, |runtime, cx| runtime.apply_config(config, cx))?;
        self.source = source;
        self.error = None;
        #[cfg(feature = "theme-watch")]
        if let Some(prepared) = prepared {
            self.run_watch(prepared, cx);
        }
        cx.notify();
        Ok(())
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) -> Result<(), StyleError> {
        let result = self.repository.load(&self.source).and_then(|config| {
            self.runtime
                .update(cx, |runtime, cx| runtime.apply_config(config, cx))
        });
        self.record_result(&result, cx);
        result
    }

    fn record_result(&mut self, result: &Result<(), StyleError>, cx: &mut Context<Self>) {
        let error = result.as_ref().err().cloned();
        if self.error != error {
            self.error = error;
            cx.notify();
        }
    }
}

#[cfg(feature = "theme-watch")]
type PreparedWatch = (
    notify::RecommendedWatcher,
    async_channel::Sender<Result<(), StyleError>>,
    async_channel::Receiver<Result<(), StyleError>>,
    std::path::PathBuf,
);

#[cfg(feature = "theme-watch")]
impl ThemeSession {
    /// Watches filesystem events without polling. Repeated calls keep the current subscription.
    pub fn start_watching(&mut self, cx: &mut Context<Self>) -> Result<(), StyleError> {
        if self.watch.is_none() {
            self.run_watch(self.prepare_watch(&self.source)?, cx);
        }
        Ok(())
    }

    pub fn stop_watching(&mut self) {
        self.task = None;
        self.watch = None;
    }

    pub fn is_watching(&self) -> bool {
        self.watch.is_some()
    }

    fn prepare_watch(&self, source: &ThemeSource) -> Result<PreparedWatch, StyleError> {
        let root = match source {
            ThemeSource::Application { app_id, .. } => {
                self.repository.application_directory(app_id)?
            }
            _ => self.repository.root().to_owned(),
        };
        let (sender, receiver) = async_channel::bounded(1);
        let watch = watch_directory(&root, sender.clone())?;
        Ok((watch, sender, receiver, root))
    }

    fn run_watch(&mut self, prepared: PreparedWatch, cx: &mut Context<Self>) {
        let (watch, sender, receiver, root) = prepared;
        self.watch = Some(watch);
        let repository = self.repository.clone();
        let source = self.source.clone();
        // Read after subscription to include changes between installation and registration.
        let _ = sender.try_send(Ok(()));
        self.task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let repository = repository.clone();
                let source = source.clone();
                let root = root.clone();
                let sender = sender.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let watch = event.and_then(|()| watch_directory(&root, sender))?;
                        Ok((watch, repository.load(&source)))
                    })
                    .await;
                if this
                    .update(cx, |session, cx| {
                        let result = result.and_then(|(watch, config)| {
                            session.watch = Some(watch);
                            config.and_then(|config| {
                                session
                                    .runtime
                                    .update(cx, |runtime, cx| runtime.apply_config(config, cx))
                            })
                        });
                        session.record_result(&result, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
}

#[cfg(feature = "theme-watch")]
fn watch_directory(
    root: &std::path::Path,
    sender: async_channel::Sender<Result<(), StyleError>>,
) -> Result<notify::RecommendedWatcher, StyleError> {
    use notify::{RecursiveMode, Watcher};
    let observed = root.to_owned();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let result = match result {
            Ok(event) if matches!(event.kind, notify::EventKind::Access(_)) => return,
            Ok(event)
                if event.paths.is_empty()
                    || event
                        .paths
                        .iter()
                        .any(|p| p.starts_with(&observed) || observed.starts_with(p)) =>
            {
                Ok(())
            }
            Ok(_) => return,
            Err(error) => Err(StyleError::new("theme watcher", error.to_string())),
        };
        let _ = sender.try_send(result);
    })
    .map_err(|e| StyleError::new("theme watcher", e.to_string()))?;
    // Watching the parent also detects atomic replacement of the theme directory itself.
    let parent = root
        .parent()
        .unwrap_or(root)
        .ancestors()
        .find(|p| p.is_dir())
        .ok_or_else(|| StyleError::new("theme watcher", "no existing parent directory"))?;
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .map_err(|e| StyleError::new(parent.display().to_string(), e.to_string()))?;
    if root.is_dir() && root != parent {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| StyleError::new(root.display().to_string(), e.to_string()))?;
    }
    Ok(watcher)
}
