use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, Decibels, Tween};
use std::io::Cursor;
use std::time::Duration;
use tracing::error;

use crate::{events::AudioEvent, game_files};

#[derive(Resource)]
pub struct Audio {
    _manager: AudioManager,
    music_track: TrackHandle,
    sfx_track: TrackHandle,
    cache: HashMap<String, StaticSoundData>,
    music: Option<StaticSoundHandle>,
}

impl Default for Audio {
    fn default() -> Self {
        let mut manager = AudioManager::new(Default::default()).unwrap();
        let music_track = manager.add_sub_track(TrackBuilder::default()).unwrap();
        let sfx_track = manager.add_sub_track(TrackBuilder::default()).unwrap();
        Self {
            _manager: manager,
            music_track,
            sfx_track,
            cache: Default::default(),
            music: Default::default(),
        }
    }
}

impl Audio {
    pub fn get_or_load_from_fn<F>(&mut self, id: &str, f: F) -> anyhow::Result<StaticSoundData>
    where
        F: FnOnce() -> Option<Vec<u8>>,
    {
        if !self.cache.contains_key(id) {
            let data = StaticSoundData::from_cursor(Cursor::new(
                f().ok_or_else(|| anyhow::anyhow!("Failed to load sound data"))?,
            ))
            .unwrap();
            self.cache.insert(id.to_string(), data);
        }
        Ok(self.cache.get(id).unwrap().clone())
    }
}

pub fn play_sound(
    mut audio_events: MessageReader<AudioEvent>,
    audio: Option<ResMut<Audio>>,
    files: Res<game_files::GameFiles>,
) {
    let mut audio = match audio {
        Some(audio) => audio,
        None => return,
    };

    for event in audio_events.read() {
        match event {
            AudioEvent::PlaySound(sound) => match *sound {
                packets::server::Sound::Sound(id) => {
                    let path = &format!("Legend/{}.mp3", id);
                    let data = audio.get_or_load_from_fn(path, || files.get_file(path));

                    if let Ok(data) = data {
                        if audio
                            .sfx_track
                            .play(data.with_settings(StaticSoundSettings::new()))
                            .is_err()
                        {
                            error!("Failed to play sound {:?}", sound);
                        }
                    } else {
                        error!("Failed to load sound {:?}", sound);
                        continue;
                    }
                }
                packets::server::Sound::Music(id) => {
                    let path = &format!("music/{}.mus", id);
                    let data = audio.get_or_load_from_fn(path, || files.get_file(path));

                    if let Ok(data) = data {
                        if let Some(mut handle) = audio.music.take() {
                            handle.stop(Tween {
                                duration: Duration::from_millis(500),
                                ..Default::default()
                            });
                        }

                        match audio.music_track.play(
                            data.with_settings(StaticSoundSettings::new().loop_region(0.0..)),
                        ) {
                            Ok(handle) => {
                                audio.music = Some(handle);
                            }
                            _ => error!("Failed to play sound {:?}", sound),
                        };
                    } else {
                        error!("Failed to load music {:?}", sound);
                        continue;
                    }
                }
            },
            _ => {}
        }
    }
}

fn amplitude_to_db(amplitude: f32) -> Decibels {
    if amplitude <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * amplitude.log10())
    }
}

pub fn sync_audio_settings(settings: Res<crate::settings::Settings>, audio: Option<ResMut<Audio>>) {
    let mut audio = match audio {
        Some(audio) => audio,
        None => return,
    };

    if settings.is_changed() {
        let _ = audio
            .music_track
            .set_volume(amplitude_to_db(settings.audio.music_volume), Tween::default());
        let _ = audio
            .sfx_track
            .set_volume(amplitude_to_db(settings.audio.sfx_volume), Tween::default());
    }
}

pub fn setup_audio_settings(
    settings: Res<crate::settings::Settings>,
    audio: Option<ResMut<Audio>>,
) {
    let mut audio = match audio {
        Some(audio) => audio,
        None => return,
    };

    let _ = audio
        .music_track
        .set_volume(amplitude_to_db(settings.audio.music_volume), Tween::default());
    let _ = audio
        .sfx_track
        .set_volume(amplitude_to_db(settings.audio.sfx_volume), Tween::default());
}
