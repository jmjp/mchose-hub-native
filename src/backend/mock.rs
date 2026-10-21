//! In-memory audio backend used for tests and for UI development without a
//! C-Media driver. Every change is applied to the in-memory [`AudioState`].

use crate::backend::{AudioBackend, BackendError, BackendKind, Result};
use crate::model::audio::{
    AudioChange, AudioState, Capabilities, EffectKind, TestChannel,
};

/// Development backend (no hardware I/O).
#[derive(Debug, Default)]
pub struct MockAudioBackend {
    state: AudioState,
    test_channel: Option<TestChannel>,
}

impl MockAudioBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioBackend for MockAudioBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Mock
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::all()
    }

    fn read(&mut self) -> Result<AudioState> {
        Ok(self.state.clone())
    }

    fn apply(&mut self, change: AudioChange) -> Result<()> {
        match change {
            AudioChange::EqEnabled(enabled) => self.state.eq_enabled = enabled,
            AudioChange::EqBand { band, value } => {
                if band >= self.state.eq_bands.len() {
                    return Err(BackendError::Unsupported("equalizer band out of range"));
                }
                self.state.eq_bands[band] = value;
            }
            AudioChange::EffectEnabled { kind, enabled } => {
                self.state.effect_mut(kind).enabled = enabled;
            }
            AudioChange::EffectLevel { kind, level } => {
                let effect = self.state.effect_mut(kind);
                effect.level = level;
                if kind == EffectKind::AdaptiveVolume {
                    effect.mode = 0;
                }
            }
            AudioChange::BassCutoffHz(hz) => {
                self.state.effect_mut(EffectKind::VirtualBass).cutoff_hz = hz
            }
            AudioChange::AdaptiveMode(mode) => {
                self.state.effect_mut(EffectKind::AdaptiveVolume).mode = mode
            }
            AudioChange::VoiceNoiseSuppression(value) => {
                self.state.effect_mut(EffectKind::VoiceClarity).noise_suppression = value
            }
            AudioChange::MagicVoiceEnabled(enabled) => self.state.mic.magic_voice = enabled,
            AudioChange::MagicVoiceType(kind) => self.state.mic.magic_voice_type = kind,
            AudioChange::AiNoiseReduction(enabled) => self.state.mic.ai_noise_reduction = enabled,
            AudioChange::NoiseSuppression(enabled) => self.state.mic.noise_suppression = enabled,
            AudioChange::SpeakerVolume(value) => {
                self.state.volume.speaker = value.clamp(0.0, 1.0)
            }
            AudioChange::SpeakerMute(muted) => self.state.volume.speaker_muted = muted,
            AudioChange::MicVolume(value) => self.state.volume.mic = value.clamp(0.0, 1.0),
            AudioChange::MicMute(muted) => self.state.volume.mic_muted = muted,
            AudioChange::SurroundEnabled(enabled) => self.state.surround.enabled = enabled,
            AudioChange::SurroundMode(mode) => self.state.surround.mode = mode,
            AudioChange::SurroundRoom(room) => self.state.surround.room = room,
            AudioChange::SurroundAmplification(enabled) => {
                self.state.surround_amplification = enabled
            }
            AudioChange::MicListen(enabled) => self.state.mic.mic_listen = enabled,
        }
        Ok(())
    }

    fn test_sound(&mut self, channel: TestChannel) -> Result<()> {
        self.test_channel = Some(channel);
        log::info!("mock test sound: {channel:?}");
        Ok(())
    }

    fn stop_test_sound(&mut self) -> Result<()> {
        self.test_channel = None;
        Ok(())
    }
}
