#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use futures::{
    StreamExt,
    future::{Either, select},
};
use gpui_notifications::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

#[derive(Clone, Debug)]
struct Sent {
    replaces: u32,
    icon: String,
    body: String,
    actions: Vec<String>,
}
struct Server(Arc<Mutex<Vec<Sent>>>);
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&str> {
        vec!["actions", "body", "body-markup"]
    }
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app: &str,
        replaces: u32,
        icon: &str,
        _title: &str,
        body: &str,
        actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _timeout: i32,
    ) -> u32 {
        self.0.lock().unwrap().push(Sent {
            replaces,
            icon: icon.into(),
            body: body.into(),
            actions,
        });
        42
    }
    fn close_notification(&self, id: u32) {
        assert_eq!(id, 42);
    }
}
async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
    match select(
        Box::pin(future),
        Box::pin(async_io::Timer::after(Duration::from_secs(5))),
    )
    .await
    {
        Either::Left((result, _)) => result,
        Either::Right(_) => panic!("system callback timed out"),
    }
}

#[test]
#[ignore = "run with GPUI_NOTIFICATION_TEST_BUS=1 dbus-run-session -- cargo test -p gpui_notifications --test linux_protocol -- --ignored"]
fn notifications_and_media_round_trip_over_private_bus() {
    assert_eq!(
        std::env::var("GPUI_NOTIFICATION_TEST_BUS").as_deref(),
        Ok("1")
    );
    async_io::block_on(async {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let server = zbus::connection::Builder::session()
            .unwrap()
            .name("org.freedesktop.Notifications")
            .unwrap()
            .serve_at("/org/freedesktop/Notifications", Server(sent.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        let center = NotificationCenter::new(NotificationOptions::new("test.gpui", "Test"))
            .await
            .unwrap();
        let events = center.take_events().unwrap();
        let mut item = Notification::new("message", "Title", "你好 <&>");
        item.icon = Some(NotificationIcon::Resource("mail-message-new".into()));
        item.actions.push(NotificationAction::new("open", "Open"));
        center.show(item.clone()).await.unwrap();
        center.show(item).await.unwrap();
        {
            let messages = sent.lock().unwrap();
            assert_eq!(messages[0].replaces, 0);
            assert_eq!(messages[1].replaces, 42);
            assert_eq!(messages[1].icon, "mail-message-new");
            assert_eq!(messages[1].body, "你好 &lt;&amp;&gt;");
            assert_eq!(messages[1].actions, ["default", "Open", "open", "Open"]);
        }
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
                "ActionInvoked",
                &(42u32, "open"),
            )
            .await
            .unwrap();
        assert_eq!(
            within(events.recv()).await.unwrap(),
            NotificationEvent::Activated {
                id: "message".into(),
                action: Some("open".into()),
                reply: None
            }
        );
        center.remove("message").await.unwrap();
        drop(center);
        assert!(within(events.recv()).await.is_err());

        let mut media = SystemMediaSession::new(MediaSessionOptions {
            app_id: "test.gpui".into(),
            app_name: "Test".into(),
        })
        .await
        .unwrap();
        let commands = media.take_commands().unwrap();
        let cover = MediaArtwork::from_rgba(1024, 4, vec![255; 1024 * 4 * 4]).unwrap();
        let normalized = image::load_from_memory(cover.png()).unwrap();
        assert_eq!((normalized.width(), normalized.height()), (512, 2));
        media.set_artwork(Some(cover)).unwrap();
        media
            .update(MediaSessionState {
                metadata: MediaMetadata {
                    track_id: "track1".into(),
                    title: "Title".into(),
                    ..Default::default()
                },
                playback: MediaPlayback::Paused,
                duration: Some(Duration::from_secs(30)),
                seekable: true,
                ..Default::default()
            })
            .unwrap();
        let client = Connection::session().await.unwrap();
        let name = format!(
            "org.mpris.MediaPlayer2.test.gpui.instance{}",
            std::process::id()
        );
        let player = Proxy::new(
            &client,
            name.as_str(),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
        )
        .await
        .unwrap();
        player.call::<_, _, ()>("Play", &()).await.unwrap();
        assert_eq!(within(commands.recv()).await.unwrap(), MediaCommand::Play);
        let metadata: HashMap<String, OwnedValue> = player.get_property("Metadata").await.unwrap();
        let track =
            OwnedObjectPath::try_from(metadata.get("mpris:trackid").unwrap().try_clone().unwrap())
                .unwrap();
        let cover_uri =
            String::try_from(metadata.get("mpris:artUrl").unwrap().try_clone().unwrap()).unwrap();
        let cover_path = url::Url::parse(&cover_uri).unwrap().to_file_path().unwrap();
        assert!(cover_path.exists());
        player
            .call::<_, _, ()>("SetPosition", &(track, 5_000_000i64))
            .await
            .unwrap();
        assert_eq!(
            within(commands.recv()).await.unwrap(),
            MediaCommand::SeekTo(Duration::from_secs(5))
        );
        player.set_property("Volume", &0.25f64).await.unwrap();
        assert_eq!(
            within(commands.recv()).await.unwrap(),
            MediaCommand::SetVolume(0.25)
        );
        let mut signals = player.receive_signal("Seeked").await.unwrap();
        media.seeked(Duration::from_secs(5)).unwrap();
        assert_eq!(
            within(signals.next())
                .await
                .unwrap()
                .body()
                .deserialize::<(i64,)>()
                .unwrap(),
            (5_000_000,)
        );
        drop(media);
        assert!(within(commands.recv()).await.is_err());
        within(async {
            while cover_path.exists() {
                async_io::Timer::after(Duration::from_millis(5)).await;
            }
        })
        .await;
    });
}
