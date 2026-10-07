use super::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zbus::{
    Connection,
    zvariant::{ObjectPath, OwnedValue, Str, Value},
};
const PATH: &str = "/org/mpris/MediaPlayer2";
struct Shared {
    state: Mutex<MediaSessionState>,
    commands: async_channel::Sender<MediaCommand>,
    updated: Mutex<std::time::Instant>,
}
struct Root {
    app_id: String,
    name: String,
}
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported(
            "Raise is not provided".into(),
        ))
    }
    fn quit(&self) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported(
            "Quit is not provided".into(),
        ))
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        &self.name
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        &self.app_id
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}
struct Player(Arc<Shared>);
impl Player {
    fn command(&self, command: MediaCommand) -> zbus::fdo::Result<()> {
        self.0
            .commands
            .try_send(command)
            .map_err(|_| zbus::fdo::Error::Failed("Media session closed".into()))
    }
}
fn metadata(state: &MediaSessionState) -> HashMap<String, OwnedValue> {
    let mut info = HashMap::new();
    info.insert(
        "mpris:trackid".into(),
        OwnedValue::try_from(Value::from(track_path(&state.metadata.track_id))).unwrap(),
    );
    info.insert(
        "xesam:title".into(),
        Str::from(state.metadata.title.clone()).into(),
    );
    if let Some(artist) = &state.metadata.artist {
        info.insert(
            "xesam:artist".into(),
            zbus::zvariant::Array::from(vec![artist.clone()])
                .try_into()
                .unwrap(),
        );
    }
    if let Some(album) = &state.metadata.album {
        info.insert("xesam:album".into(), Str::from(album.clone()).into());
    }
    if let Some(duration) = state.duration {
        info.insert("mpris:length".into(), (duration.as_micros() as i64).into());
    }
    info
}
fn track_path(id: &str) -> zbus::zvariant::OwnedObjectPath {
    use std::fmt::Write;
    let mut path = String::from("/org/gpui/track_");
    for byte in id.bytes() {
        let _ = write!(&mut path, "{byte:02x}");
    }
    zbus::zvariant::OwnedObjectPath::try_from(path).unwrap()
}
fn status(state: &MediaSessionState) -> &'static str {
    match state.playback {
        MediaPlayback::Playing => "Playing",
        MediaPlayback::Paused | MediaPlayback::Buffering => "Paused",
        MediaPlayback::Stopped => "Stopped",
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn play(&self) -> zbus::fdo::Result<()> {
        self.command(MediaCommand::Play)
    }
    fn pause(&self) -> zbus::fdo::Result<()> {
        self.command(MediaCommand::Pause)
    }
    fn play_pause(&self) -> zbus::fdo::Result<()> {
        self.command(MediaCommand::Toggle)
    }
    fn stop(&self) -> zbus::fdo::Result<()> {
        self.command(MediaCommand::Stop)
    }
    fn next(&self) -> zbus::fdo::Result<()> {
        if !self.0.state.lock().unwrap().can_next {
            return Ok(());
        }
        self.command(MediaCommand::Next)
    }
    fn previous(&self) -> zbus::fdo::Result<()> {
        if !self.0.state.lock().unwrap().can_previous {
            return Ok(());
        }
        self.command(MediaCommand::Previous)
    }
    fn seek(&self, offset: i64) -> zbus::fdo::Result<()> {
        if !self.can_seek() {
            return Ok(());
        }
        self.command(MediaCommand::SeekBy(offset as f64 / 1_000_000.))
    }
    fn set_position(&self, track_id: ObjectPath<'_>, position: i64) -> zbus::fdo::Result<()> {
        let state = self.0.state.lock().unwrap();
        if track_id.as_str() != track_path(&state.metadata.track_id).as_str()
            || position < 0
            || state
                .duration
                .is_some_and(|d| position as u128 > d.as_micros())
        {
            return Ok(());
        }
        if !state.seekable {
            return Ok(());
        }
        self.command(MediaCommand::SeekTo(Duration::from_micros(position as u64)))
    }
    fn open_uri(&self, _uri: &str) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported(
            "OpenUri is not provided".into(),
        ))
    }
    #[zbus(property)]
    fn playback_status(&self) -> String {
        status(&self.0.state.lock().unwrap()).into()
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        metadata(&self.0.state.lock().unwrap())
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        let state = self.0.state.lock().unwrap();
        let elapsed = if state.playback == MediaPlayback::Playing {
            self.0.updated.lock().unwrap().elapsed().as_secs_f64() * state.rate
        } else {
            0.
        };
        let seconds = (state.position.as_secs_f64() + elapsed)
            .min(state.duration.map_or(f64::MAX, |d| d.as_secs_f64()));
        (seconds * 1_000_000.) as i64
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.0.state.lock().unwrap().rate
    }
    #[zbus(property)]
    fn set_rate(&self, value: f64) -> zbus::Result<()> {
        if value == 0. {
            self.command(MediaCommand::Pause)?;
        }
        Ok(())
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.0.state.lock().unwrap().volume
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) -> zbus::Result<()> {
        if value.is_finite() {
            self.command(MediaCommand::SetVolume(value.clamp(0., 1.)))?;
        }
        Ok(())
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        self.rate().min(1.)
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        self.rate().max(1.)
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.0.state.lock().unwrap().can_next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.0.state.lock().unwrap().can_previous
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.0.state.lock().unwrap().seekable
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

struct LinuxMedia {
    shared: Arc<Shared>,
    changed: async_channel::Sender<()>,
    seeks: async_channel::Sender<Duration>,
}
pub async fn create(options: MediaSessionOptions) -> Result<SystemMediaSession> {
    let (commands, rx) = async_channel::unbounded();
    let shared = Arc::new(Shared {
        state: Mutex::new(MediaSessionState::default()),
        commands,
        updated: Mutex::new(std::time::Instant::now()),
    });
    let connection = zbus::connection::Builder::session()?
        .name(format!(
            "org.mpris.MediaPlayer2.{}.instance{}",
            options.app_id,
            std::process::id()
        ))?
        .serve_at(
            PATH,
            Root {
                app_id: options.app_id,
                name: options.app_name,
            },
        )?
        .serve_at(PATH, Player(shared.clone()))?
        .build()
        .await?;
    let (changed, changes) = async_channel::bounded(1);
    let (seeks, positions) = async_channel::unbounded::<Duration>();
    let state = shared.clone();
    std::thread::Builder::new()
        .name("gpui-media-controls".into())
        .spawn(move || {
            async_io::block_on(async move {
                loop {
                    match futures::future::select(
                        Box::pin(changes.recv()),
                        Box::pin(positions.recv()),
                    )
                    .await
                    {
                        futures::future::Either::Left((Ok(()), _)) => {
                            let snapshot = state.state.lock().unwrap().clone();
                            let _ = publish(&connection, &snapshot).await;
                        }
                        futures::future::Either::Right((Ok(position), _)) => {
                            let _ = connection
                                .emit_signal(
                                    None::<&str>,
                                    PATH,
                                    "org.mpris.MediaPlayer2.Player",
                                    "Seeked",
                                    &(position.as_micros() as i64,),
                                )
                                .await;
                        }
                        _ => break,
                    }
                }
            })
        })?;
    Ok(SystemMediaSession::from_backend(
        Box::new(LinuxMedia {
            shared,
            changed,
            seeks,
        }),
        rx,
    ))
}
async fn publish(connection: &Connection, state: &MediaSessionState) -> Result<()> {
    let mut properties: HashMap<&str, OwnedValue> = HashMap::new();
    properties.insert("PlaybackStatus", Str::from(status(state)).into());
    properties.insert("Metadata", metadata(state).try_into()?);
    properties.insert("Rate", state.rate.into());
    properties.insert("Volume", state.volume.into());
    properties.insert("MinimumRate", state.rate.min(1.).into());
    properties.insert("MaximumRate", state.rate.max(1.).into());
    properties.insert("CanSeek", state.seekable.into());
    properties.insert("CanGoNext", state.can_next.into());
    properties.insert("CanGoPrevious", state.can_previous.into());
    connection
        .emit_signal(
            None::<&str>,
            PATH,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.mpris.MediaPlayer2.Player",
                properties,
                Vec::<String>::new(),
            ),
        )
        .await?;
    Ok(())
}
impl MediaSessionBackend for LinuxMedia {
    fn seeked(&self, position: Duration) -> Result<()> {
        self.seeks.try_send(position)?;
        Ok(())
    }
    fn update(&self, state: MediaSessionState) -> Result<()> {
        {
            let mut current = self.shared.state.lock().unwrap();
            *current = state;
            *self.shared.updated.lock().unwrap() = std::time::Instant::now();
        }
        match self.changed.try_send(()) {
            Ok(()) | Err(async_channel::TrySendError::Full(())) => Ok(()),
            Err(_) => anyhow::bail!("media session closed"),
        }
    }
}
impl Drop for LinuxMedia {
    fn drop(&mut self) {
        self.changed.close();
        self.seeks.close();
        self.shared.commands.close();
    }
}
