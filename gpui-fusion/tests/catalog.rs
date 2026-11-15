use gpui_fusion::{
    default_theme,
    style::MaterialMode,
    theme::ThemeConfig,
    theme::catalog::{ThemeCatalog, ThemeEntry, ThemeIndex, ThemeRepository, ThemeSource},
};
use std::{collections::BTreeMap, path::Path};

fn config(mode: &str, material: MaterialMode) -> ThemeConfig {
    let mut theme = default_theme();
    theme.material = material;
    ThemeConfig {
        theme,
        mode: mode.into(),
        style_set: "neutral".into(),
    }
}

fn write_catalog(directory: &Path, selected: Option<&str>) {
    std::fs::create_dir_all(directory.join("variants")).unwrap();
    let themes = BTreeMap::from([
        (
            "daybreak".into(),
            ThemeEntry {
                name: "Daybreak".into(),
                file: "variants/daybreak.json".into(),
            },
        ),
        (
            "midnight".into(),
            ThemeEntry {
                name: "Midnight".into(),
                file: "variants/midnight.json".into(),
            },
        ),
    ]);
    let index = ThemeIndex {
        default: selected.map(String::from),
        themes,
    };
    std::fs::write(
        directory.join("index.toml"),
        toml::to_string(&index).unwrap(),
    )
    .unwrap();
    for (id, mode, material) in [
        ("daybreak", "light", MaterialMode::Plain),
        ("midnight", "dark", MaterialMode::Glass),
    ] {
        std::fs::write(
            directory.join(format!("variants/{id}.json")),
            serde_json::to_string(&config(mode, material)).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn application_default_overrides_global_and_pinned_selection_remains_independent() {
    let temp = tempfile::tempdir().unwrap();
    let repository = ThemeRepository::from_root(temp.path()).unwrap();
    let follow = ThemeSource::Follow {
        app_id: "org.fusion.catalog-test".into(),
    };
    assert_eq!(
        repository.load(&follow).unwrap(),
        config("light", MaterialMode::Plain)
    );
    write_catalog(temp.path(), Some("midnight"));
    assert_eq!(
        repository.load(&follow).unwrap(),
        config("dark", MaterialMode::Glass)
    );
    let directory = repository
        .application_directory("org.fusion.catalog-test")
        .unwrap();
    write_catalog(&directory, None);
    assert_eq!(
        repository.load(&follow).unwrap(),
        config("dark", MaterialMode::Glass)
    );
    let catalog = repository
        .application_catalog("org.fusion.catalog-test")
        .unwrap();
    assert_eq!(
        catalog.load("daybreak").unwrap(),
        config("light", MaterialMode::Plain)
    );
    let pinned = ThemeSource::Application {
        app_id: "org.fusion.catalog-test".into(),
        theme_id: "midnight".into(),
    };
    write_catalog(&directory, Some("daybreak"));
    assert_eq!(
        repository.load(&follow).unwrap(),
        config("light", MaterialMode::Plain)
    );
    assert_eq!(
        repository.load(&pinned).unwrap(),
        config("dark", MaterialMode::Glass)
    );
    std::fs::remove_file(directory.join("index.toml")).unwrap();
    assert_eq!(
        repository.load(&follow).unwrap(),
        config("dark", MaterialMode::Glass)
    );
    assert!(repository.load(&pinned).is_err());
}

#[test]
fn broken_application_configuration_is_reported_instead_of_using_global() {
    let temp = tempfile::tempdir().unwrap();
    write_catalog(temp.path(), Some("daybreak"));
    let repository = ThemeRepository::from_root(temp.path()).unwrap();
    let directory = repository
        .application_directory("org.fusion.catalog-test")
        .unwrap();
    write_catalog(&directory, Some("midnight"));
    let follow = ThemeSource::Follow {
        app_id: "org.fusion.catalog-test".into(),
    };
    std::fs::write(directory.join("variants/midnight.json"), "{").unwrap();
    assert!(repository.load(&follow).is_err());
    std::fs::remove_file(directory.join("variants/midnight.json")).unwrap();
    assert!(repository.load(&follow).is_err());
    std::fs::write(directory.join("index.toml"), "default =").unwrap();
    assert!(repository.load(&follow).is_err());
}

#[test]
fn index_validation_rejects_missing_defaults_and_paths_outside_catalog() {
    let temp = tempfile::tempdir().unwrap();
    write_catalog(temp.path(), Some("daybreak"));
    let mut index = ThemeCatalog::open(temp.path()).unwrap().index().clone();
    index.default = Some("unregistered".into());
    assert!(ThemeIndex::from_toml(&toml::to_string(&index).unwrap()).is_err());
    index.default = None;
    for path in ["../external.json", "/external.json", ""] {
        index.themes.get_mut("daybreak").unwrap().file = path.into();
        assert!(ThemeIndex::from_toml(&toml::to_string(&index).unwrap()).is_err());
    }
    let repository = ThemeRepository::from_root(temp.path()).unwrap();
    for id in ["", ".", "..", "one/two", "one\\two", "/root"] {
        assert!(repository.application_directory(id).is_err());
    }
}

#[gpui::test]
fn session_reload_is_atomic_and_reuses_the_injected_runtime(cx: &mut gpui::TestAppContext) {
    use gpui_fusion::theme::session::ThemeSession;
    let temp = tempfile::tempdir().unwrap();
    write_catalog(temp.path(), Some("daybreak"));
    let repository = ThemeRepository::from_root(temp.path()).unwrap();
    let session = cx.update(|cx| {
        ThemeSession::install(
            repository,
            ThemeSource::Follow {
                app_id: "org.fusion.session-test".into(),
            },
            cx,
        )
        .unwrap()
    });
    let runtime = cx.update(|cx| session.read(cx).runtime().clone());
    let pinned = ThemeSource::Global {
        theme_id: "midnight".into(),
    };
    cx.update(|cx| {
        session.update(cx, |session, cx| {
            session.set_source(pinned.clone(), cx).unwrap()
        })
    });
    cx.update(|cx| {
        assert_eq!(
            runtime.read(cx).configuration(),
            config("dark", MaterialMode::Glass)
        )
    });
    std::fs::write(temp.path().join("variants/midnight.json"), "{").unwrap();
    cx.update(|cx| {
        session.update(cx, |session, cx| {
            assert!(session.reload(cx).is_err());
            assert!(session.error().is_some());
            assert_eq!(session.source(), &pinned);
            assert_eq!(session.runtime().entity_id(), runtime.entity_id());
            assert_eq!(
                runtime.read(cx).configuration(),
                config("dark", MaterialMode::Glass)
            );
            assert!(
                session
                    .set_source(
                        ThemeSource::Global {
                            theme_id: "unregistered".into()
                        },
                        cx
                    )
                    .is_err()
            );
            assert_eq!(session.source(), &pinned);
        })
    });
    write_catalog(temp.path(), Some("daybreak"));
    cx.update(|cx| {
        session.update(cx, |session, cx| {
            session.reload(cx).unwrap();
            assert!(session.error().is_none());
            assert_eq!(
                runtime.read(cx).configuration(),
                config("dark", MaterialMode::Glass)
            );
        })
    });
}

#[cfg(all(feature = "theme-watch", target_os = "linux"))]
#[gpui::test]
fn file_subscription_handles_atomic_save_invalid_data_and_stop(cx: &mut gpui::TestAppContext) {
    use gpui_fusion::theme::session::ThemeSession;
    // Native filesystem callbacks wake the executor from the OS watcher thread.
    cx.background_executor.allow_parking();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("settings").join("appearance");
    let repository = ThemeRepository::from_root(&root).unwrap();
    let session = cx.update(|cx| {
        ThemeSession::install(
            repository,
            ThemeSource::Follow {
                app_id: "org.fusion.watch-test".into(),
            },
            cx,
        )
        .unwrap()
    });
    let runtime = cx.update(|cx| session.read(cx).runtime().clone());
    cx.update(|cx| session.update(cx, |s, cx| s.start_watching(cx).unwrap()));
    cx.run_until_parked();
    write_catalog(&root, Some("midnight"));
    wait_for(cx, |cx| runtime.read(cx).store().mode() == "dark");
    let file = root.join("variants/midnight.json");
    std::fs::write(&file, "{").unwrap();
    wait_for(cx, |cx| session.read(cx).error().is_some());
    cx.update(|cx| {
        assert_eq!(
            runtime.read(cx).configuration(),
            config("dark", MaterialMode::Glass)
        )
    });
    let replacement = root.join("variants/replacement.json");
    std::fs::write(
        &replacement,
        serde_json::to_string(&config("light", MaterialMode::Glass)).unwrap(),
    )
    .unwrap();
    std::fs::rename(replacement, file).unwrap();
    wait_for(cx, |cx| {
        session.read(cx).error().is_none() && runtime.read(cx).store().mode() == "light"
    });
    cx.update(|cx| session.update(cx, |s, _| s.stop_watching()));
    write_catalog(&root, Some("midnight"));
    std::thread::sleep(std::time::Duration::from_millis(40));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(runtime.read(cx).store().mode(), "light"));
    cx.update(|cx| session.update(cx, |s, cx| s.reload(cx).unwrap()));
    cx.update(|cx| assert_eq!(runtime.read(cx).store().mode(), "dark"));
}

#[cfg(all(feature = "theme-watch", target_os = "linux"))]
fn wait_for(cx: &gpui::TestAppContext, predicate: impl Fn(&gpui::App) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        cx.run_until_parked();
        if cx.update(|cx| predicate(cx)) {
            return;
        }
        assert!(
            std::time::Instant::now() < until,
            "theme subscription did not converge"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
