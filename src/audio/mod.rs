use std::collections::HashMap;
use std::path::Path;
use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::DefaultBackend},
    sound::{static_sound::{StaticSoundData, StaticSoundSettings, StaticSoundHandle}, PlaybackRate},
    tween::Tween,
    Volume,
};

/// Configuration for playing a sound with variation.
#[derive(Debug, Clone, Copy)]
pub struct SoundConfig {
    pub volume: f32,
    pub pitch: f32,
    /// Random pitch variation range (e.g. 0.1 = +/- 10%)
    pub pitch_variation: f32,
    /// Random volume variation range
    pub volume_variation: f32,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self { volume: 1.0, pitch: 1.0, pitch_variation: 0.0, volume_variation: 0.0 }
    }
}

pub struct AudioContext {
    /// `None` when audio hardware is unavailable (headless / CI / no audio device).
    manager: Option<AudioManager>,
    sounds: HashMap<String, StaticSoundData>,
    active_music: Option<StaticSoundHandle>,
    time_seed: u64,
}

impl AudioContext {
    pub fn new() -> Self {
        let manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("[audio] Failed to initialize audio manager: {e}. Audio disabled.");
                None
            }
        };
        Self {
            manager,
            sounds: HashMap::new(),
            active_music: None,
            time_seed: 0,
        }
    }

    /// Returns true if audio hardware is available.
    pub fn is_available(&self) -> bool { self.manager.is_some() }

    /// Load a sound file (OGG, WAV, etc.) into memory.
    /// Logs a warning and returns if the file cannot be read.
    pub fn load_sound<P: AsRef<Path>>(&mut self, name: &str, path: P) {
        match StaticSoundData::from_file(path.as_ref()) {
            Ok(sound) => { self.sounds.insert(name.to_string(), sound); }
            Err(e) => eprintln!("[audio] Failed to load '{}' from '{}': {e}", name, path.as_ref().display()),
        }
    }

    /// Play a sound once with optional config.
    pub fn play(&mut self, name: &str, config: SoundConfig) {
        let Some(manager) = self.manager.as_mut() else { return; };
        if let Some(data) = self.sounds.get(name) {
            let mut settings = StaticSoundSettings::new();

            // Advance seed independently for each random variable to avoid LCG correlation.
            self.time_seed = self.time_seed.wrapping_add(1);
            let p_offset = (pseudo_rand(self.time_seed) - 0.5) * 2.0 * config.pitch_variation;
            self.time_seed = self.time_seed.wrapping_add(1);
            let v_offset = (pseudo_rand(self.time_seed) - 0.5) * 2.0 * config.volume_variation;

            settings.playback_rate = PlaybackRate::Factor((config.pitch + p_offset) as f64).into();
            settings.volume = Volume::Amplitude((config.volume + v_offset).clamp(0.0, 2.0) as f64).into();

            let _ = manager.play(data.clone().with_settings(settings));
        }
    }

    /// Play background music that loops indefinitely.
    pub fn play_music(&mut self, name: &str, fade_in_secs: f32) {
        let Some(manager) = self.manager.as_mut() else { return; };
        if let Some(data) = self.sounds.get(name) {
            // Fade out previous music with a fixed short duration independent of the new track's fade-in.
            if let Some(mut handle) = self.active_music.take() {
                let _ = handle.stop(Tween {
                    duration: std::time::Duration::from_secs_f32(0.5),
                    ..Default::default()
                });
            }

            let mut settings = StaticSoundSettings::new().loop_region(0.0..);
            settings.volume = Volume::Amplitude(0.0).into();

            match manager.play(data.clone().with_settings(settings)) {
                Ok(mut handle) => {
                    let _ = handle.set_volume(Volume::Amplitude(1.0), Tween {
                        duration: std::time::Duration::from_secs_f32(fade_in_secs),
                        ..Default::default()
                    });
                    self.active_music = Some(handle);
                }
                Err(e) => eprintln!("[audio] Failed to play music '{name}': {e}"),
            }
        }
    }

    /// Play a sound with 2D spatial panning and distance-based volume.
    /// `source_x/y` is the world position of the sound.
    /// `listener_x/y` is usually the camera target position.
    pub fn play_spatial(&mut self, name: &str, source_x: f32, source_y: f32, listener_x: f32, listener_y: f32, max_dist: f32) {
        let Some(manager) = self.manager.as_mut() else { return; };
        if let Some(data) = self.sounds.get(name) {
            let dx = source_x - listener_x;
            let dy = source_y - listener_y;
            let dist = (dx*dx + dy*dy).sqrt();

            if dist > max_dist { return; }

            let volume = (1.0 - (dist / max_dist)).clamp(0.0, 1.0) as f64;
            // Simple panning: -1.0 (left) to 1.0 (right)
            let panning = (dx / max_dist).clamp(-1.0, 1.0) as f64;

            let mut settings = StaticSoundSettings::new();
            settings.volume = Volume::Amplitude(volume).into();
            settings.panning = panning.into();

            let _ = manager.play(data.clone().with_settings(settings));
        }
    }
}

impl Default for AudioContext {
    fn default() -> Self { Self::new() }
}

fn pseudo_rand(seed: u64) -> f32 {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (x >> 33) as f32 / u32::MAX as f32
}
