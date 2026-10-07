use super::*;
use block2::RcBlock;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSDictionary, NSNumber, NSString};
use objc2_media_player::*;
use std::{
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
struct MacMedia {
    center: Retained<MPNowPlayingInfoCenter>,
    remote: Retained<MPRemoteCommandCenter>,
    targets: Vec<(Retained<MPRemoteCommand>, Retained<AnyObject>)>,
    commands: async_channel::Sender<MediaCommand>,
}
pub async fn create(_: MediaSessionOptions) -> Result<SystemMediaSession> {
    ensure!(
        !ACTIVE.swap(true, Ordering::AcqRel),
        "macOS has one Now Playing session per application; release the active session first"
    );
    let (commands, rx) = async_channel::unbounded();
    // The native objects are retained, and each block target is removed on drop.
    unsafe {
        let center = MPNowPlayingInfoCenter::defaultCenter();
        let remote = MPRemoteCommandCenter::sharedCommandCenter();
        let mut native = MacMedia {
            center,
            remote,
            targets: Vec::new(),
            commands,
        };
        for (command, event) in [
            (native.remote.playCommand(), MediaCommand::Play),
            (native.remote.pauseCommand(), MediaCommand::Pause),
            (native.remote.stopCommand(), MediaCommand::Stop),
            (native.remote.togglePlayPauseCommand(), MediaCommand::Toggle),
            (native.remote.nextTrackCommand(), MediaCommand::Next),
            (native.remote.previousTrackCommand(), MediaCommand::Previous),
        ] {
            let sender = native.commands.clone();
            let target = command.addTargetWithHandler(&RcBlock::new(
                move |_: NonNull<MPRemoteCommandEvent>| {
                    if sender.try_send(event.clone()).is_ok() {
                        MPRemoteCommandHandlerStatus::Success
                    } else {
                        MPRemoteCommandHandlerStatus::CommandFailed
                    }
                },
            ));
            command.setEnabled(true);
            native.targets.push((command, target));
        }
        let sender = native.commands.clone();
        let command = native.remote.changePlaybackPositionCommand();
        let target = command.addTargetWithHandler(&RcBlock::new(
            move |event: NonNull<MPRemoteCommandEvent>| {
                let event = event
                    .as_ref()
                    .downcast_ref::<MPChangePlaybackPositionCommandEvent>();
                if let Some(seconds) = event
                    .map(|event| event.positionTime())
                    .filter(|seconds| seconds.is_finite() && *seconds >= 0.)
                {
                    if let Ok(position) = Duration::try_from_secs_f64(seconds) {
                        if sender.try_send(MediaCommand::SeekTo(position)).is_ok() {
                            return MPRemoteCommandHandlerStatus::Success;
                        }
                    }
                }
                MPRemoteCommandHandlerStatus::CommandFailed
            },
        ));
        native.targets.push((command.into_super(), target));
        native.update(MediaSessionState::default())?;
        Ok(SystemMediaSession::from_backend(Box::new(native), rx))
    }
}
impl MediaSessionBackend for MacMedia {
    fn update(&self, state: MediaSessionState) -> Result<()> {
        unsafe {
            let mut keys = vec![
                MPMediaItemPropertyTitle,
                MPMediaItemPropertyArtist,
                MPMediaItemPropertyAlbumTitle,
                MPNowPlayingInfoPropertyElapsedPlaybackTime,
                MPNowPlayingInfoPropertyPlaybackRate,
            ];
            let mut values: Vec<Retained<AnyObject>> = vec![
                Retained::cast_unchecked(NSString::from_str(&state.metadata.title)),
                Retained::cast_unchecked(NSString::from_str(
                    state.metadata.artist.as_deref().unwrap_or_default(),
                )),
                Retained::cast_unchecked(NSString::from_str(
                    state.metadata.album.as_deref().unwrap_or_default(),
                )),
                Retained::cast_unchecked(NSNumber::new_f64(state.position.as_secs_f64())),
                Retained::cast_unchecked(NSNumber::new_f64(
                    if state.playback == MediaPlayback::Playing {
                        state.rate
                    } else {
                        0.
                    },
                )),
            ];
            if let Some(duration) = state.duration {
                keys.push(MPMediaItemPropertyPlaybackDuration);
                values.push(Retained::cast_unchecked(NSNumber::new_f64(
                    duration.as_secs_f64(),
                )));
            }
            self.center
                .setNowPlayingInfo(Some(&NSDictionary::from_slices(
                    &keys,
                    &values.iter().map(|value| &**value).collect::<Vec<_>>(),
                )));
            self.center.setPlaybackState(match state.playback {
                MediaPlayback::Playing => MPNowPlayingPlaybackState::Playing,
                MediaPlayback::Paused => MPNowPlayingPlaybackState::Paused,
                MediaPlayback::Buffering => MPNowPlayingPlaybackState::Interrupted,
                MediaPlayback::Stopped => MPNowPlayingPlaybackState::Stopped,
            });
            self.remote
                .changePlaybackPositionCommand()
                .setEnabled(state.seekable);
            self.remote.nextTrackCommand().setEnabled(state.can_next);
            self.remote
                .previousTrackCommand()
                .setEnabled(state.can_previous);
        }
        Ok(())
    }
}
impl Drop for MacMedia {
    fn drop(&mut self) {
        unsafe {
            for (command, target) in &self.targets {
                command.removeTarget(Some(target));
                command.setEnabled(false);
            }
            self.center.setNowPlayingInfo(None);
            self.center
                .setPlaybackState(MPNowPlayingPlaybackState::Stopped);
        }
        self.commands.close();
        ACTIVE.store(false, Ordering::Release);
    }
}
