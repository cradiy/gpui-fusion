use super::*;
use ::windows::{
    Foundation::{TimeSpan, TypedEventHandler, Uri},
    Media::{Playback::MediaPlayer, *},
    Storage::Streams::RandomAccessStreamReference,
    core::HSTRING,
};
struct WindowsMedia {
    player: MediaPlayer,
    controls: SystemMediaTransportControls,
    buttons: i64,
    seek: i64,
    commands: async_channel::Sender<MediaCommand>,
    metadata: RefCell<Option<MediaMetadata>>,
}
pub async fn create(_: MediaSessionOptions) -> Result<SystemMediaSession> {
    let player = MediaPlayer::new()?;
    player.CommandManager()?.SetIsEnabled(false)?;
    let controls = player.SystemMediaTransportControls()?;
    let (commands, rx) = async_channel::unbounded();
    let sender = commands.clone();
    let buttons = controls.ButtonPressed(&TypedEventHandler::new(
        move |_,
              args: ::windows::core::Ref<
            '_,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >| {
            if let Some(args) = args.as_ref() {
                let command = match args.Button()? {
                    SystemMediaTransportControlsButton::Play => MediaCommand::Play,
                    SystemMediaTransportControlsButton::Pause => MediaCommand::Pause,
                    SystemMediaTransportControlsButton::Stop => MediaCommand::Stop,
                    SystemMediaTransportControlsButton::Next => MediaCommand::Next,
                    SystemMediaTransportControlsButton::Previous => MediaCommand::Previous,
                    _ => return Ok(()),
                };
                let _ = sender.try_send(command);
            }
            Ok(())
        },
    ))?;
    let sender = commands.clone();
    let seek = match controls.PlaybackPositionChangeRequested(&TypedEventHandler::new(
        move |_, args: ::windows::core::Ref<'_, PlaybackPositionChangeRequestedEventArgs>| {
            if let Some(args) = args.as_ref() {
                let ticks = args.RequestedPlaybackPosition()?.Duration;
                if ticks >= 0 {
                    let _ = sender.try_send(MediaCommand::SeekTo(Duration::new(
                        ticks as u64 / 10_000_000,
                        (ticks as u64 % 10_000_000) as u32 * 100,
                    )));
                }
            }
            Ok(())
        },
    )) {
        Ok(token) => token,
        Err(error) => {
            let _ = controls.RemoveButtonPressed(buttons);
            return Err(error.into());
        }
    };
    let native = WindowsMedia {
        player,
        controls,
        buttons,
        seek,
        commands,
        metadata: RefCell::default(),
    };
    native.controls.SetIsPlayEnabled(true)?;
    native.controls.SetIsPauseEnabled(true)?;
    native.controls.SetIsStopEnabled(true)?;
    native.controls.SetIsEnabled(true)?;
    Ok(SystemMediaSession::from_backend(Box::new(native), rx))
}
fn time(value: Duration) -> TimeSpan {
    TimeSpan {
        Duration: (value.as_nanos() / 100).min(i64::MAX as u128) as i64,
    }
}
impl MediaSessionBackend for WindowsMedia {
    fn set_artwork(&self, artwork: Option<MediaArtwork>) -> Result<()> {
        let display = self.controls.DisplayUpdater()?;
        let thumbnail = artwork
            .as_ref()
            .map(|artwork| {
                RandomAccessStreamReference::CreateFromUri(&Uri::CreateUri(&HSTRING::from(
                    artwork.file_url(),
                ))?)
            })
            .transpose()?;
        display.SetThumbnail(thumbnail.as_ref())?;
        display.Update()?;
        Ok(())
    }
    fn update(&self, state: MediaSessionState) -> Result<()> {
        self.controls.SetPlaybackStatus(match state.playback {
            MediaPlayback::Playing => MediaPlaybackStatus::Playing,
            MediaPlayback::Buffering => MediaPlaybackStatus::Changing,
            MediaPlayback::Paused => MediaPlaybackStatus::Paused,
            MediaPlayback::Stopped => MediaPlaybackStatus::Stopped,
        })?;
        self.controls.SetIsNextEnabled(state.can_next)?;
        self.controls.SetIsPreviousEnabled(state.can_previous)?;
        if self.metadata.borrow().as_ref() != Some(&state.metadata) {
            let display = self.controls.DisplayUpdater()?;
            display.SetType(MediaPlaybackType::Music)?;
            let music = display.MusicProperties()?;
            music.SetTitle(&HSTRING::from(&state.metadata.title))?;
            music.SetArtist(&HSTRING::from(
                state.metadata.artist.as_deref().unwrap_or_default(),
            ))?;
            music.SetAlbumTitle(&HSTRING::from(
                state.metadata.album.as_deref().unwrap_or_default(),
            ))?;
            display.Update()?;
            *self.metadata.borrow_mut() = Some(state.metadata);
        }
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(time(Duration::ZERO))?;
        timeline.SetEndTime(time(state.duration.unwrap_or(state.position)))?;
        timeline.SetMinSeekTime(time(if state.seekable {
            Duration::ZERO
        } else {
            state.position
        }))?;
        timeline.SetMaxSeekTime(time(if state.seekable {
            state.duration.unwrap_or(state.position)
        } else {
            state.position
        }))?;
        timeline.SetPosition(time(state.position))?;
        self.controls.UpdateTimelineProperties(&timeline)?;
        Ok(())
    }
}
impl Drop for WindowsMedia {
    fn drop(&mut self) {
        let _ = self.controls.SetIsEnabled(false);
        let _ = self.controls.RemoveButtonPressed(self.buttons);
        let _ = self
            .controls
            .RemovePlaybackPositionChangeRequested(self.seek);
        let _ = self.player.Close();
        self.commands.close();
    }
}
