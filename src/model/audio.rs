//! Semantic audio state shared between the UI and the audio backend.
//!
//! The original application drives these values either through C-Media ConfLib
//! (`PropertyControl`) or, for a handful of endpoint values, through the Windows
//! audio stack. The native backend here writes the same values to the Windows
//! property store (`IPropertyStore`) using keys recovered from `osConfLib.dll`,
//! and falls back to Core Audio for endpoint volume/mute.

use serde::{Deserialize, Serialize};

/// Number of bands in the legacy C-Media 10-band equalizer.
pub const EQ_BANDS: usize = 10;
/// Frequency labels (approximate C-Media 10-band centres) used by the EQ editor.
pub const EQ_FREQUENCIES_HZ: [u32; EQ_BANDS] = [
    31, 62, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000,
];
/// EQ gain limits (the shipped presets stay inside the driver's range).
// The shipped presets contain -14..=+14, so the editor range is wider than the
// driver's nominal +/-12 dB clamp (values are clamped only by the driver).
pub const EQ_GAIN_MIN: i32 = -15;
pub const EQ_GAIN_MAX: i32 = 15;

/// Effect blocks available in the legacy C-Media layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    /// 高频增强 / treble enhancement (`Enable_AUDIOBRILLIANT_LFX`, `AUDIOBRILLIANT_LEVEL`).
    Brilliance,
    /// 低音增强 / dynamic low frequency (`Enable_VIRTUALBASS_LFX`, level + cutoff).
    VirtualBass,
    /// 音量均衡 / volume balance (`Enable_ADAPTIVEVOLUME_GFX`, level + mode).
    AdaptiveVolume,
    /// 人声增强 / vocal clarity (`Enable_VOICECLARITY_LFX`, level + noise suppression).
    VoiceClarity,
}

impl EffectKind {
    pub const ALL: [EffectKind; 4] = [
        EffectKind::Brilliance,
        EffectKind::VirtualBass,
        EffectKind::AdaptiveVolume,
        EffectKind::VoiceClarity,
    ];

    pub fn title_key(self) -> &'static str {
        match self {
            EffectKind::Brilliance => "effect.brilliance",
            EffectKind::VirtualBass => "effect.bass",
            EffectKind::AdaptiveVolume => "effect.balance",
            EffectKind::VoiceClarity => "effect.clarity",
        }
    }

    pub fn description_key(self) -> &'static str {
        match self {
            EffectKind::Brilliance => "effect.brilliance.desc",
            EffectKind::VirtualBass => "effect.bass.desc",
            EffectKind::AdaptiveVolume => "effect.balance.desc",
            EffectKind::VoiceClarity => "effect.clarity.desc",
        }
    }
}

/// A single effect block's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectState {
    pub enabled: bool,
    pub level: i32,
    /// Virtual-bass cutoff frequency in Hz (ignored by other blocks).
    pub cutoff_hz: i32,
    /// Adaptive-volume mode: 0 = music (high dynamics), 1 = voice (low dynamics).
    pub mode: u8,
    /// Voice-clarity noise suppression level in dB (0..=20, step 5).
    pub noise_suppression: i32,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            enabled: false,
            level: 0,
            cutoff_hz: 200,
            mode: 0,
            noise_suppression: 0,
        }
    }
}

/// Virtual 7.1 surround (Xear) settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurroundMode {
    Music,
    Movie,
}

/// Room size presets for the virtual surround.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomSize {
    Small,
    Medium,
    Large,
}

impl RoomSize {
    pub fn label_key(self) -> &'static str {
        match self {
            RoomSize::Small => "surround.room.small",
            RoomSize::Medium => "surround.room.mid",
            RoomSize::Large => "surround.room.big",
        }
    }
}

/// Surround state. `None`-capable values mean "not readable from the device".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurroundState {
    pub enabled: bool,
    pub mode: SurroundMode,
    pub room: RoomSize,
}

impl Default for SurroundState {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: SurroundMode::Music,
            room: RoomSize::Medium,
        }
    }
}

/// Microphone DSP state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MicState {
    pub ai_noise_reduction: bool,
    pub noise_suppression: bool,
    pub magic_voice: bool,
    /// 0 = monster, 1 = cartoon, 2 = male, 3 = female (vendor selection values).
    pub magic_voice_type: u8,
    pub mic_listen: bool,
}

/// Endpoint volume state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VolumeState {
    /// 0.0..=1.0 speaker (render) volume.
    pub speaker: f32,
    pub speaker_muted: bool,
    /// 0.0..=1.0 microphone (capture) volume.
    pub mic: f32,
    pub mic_muted: bool,
}

impl Default for VolumeState {
    fn default() -> Self {
        Self {
            speaker: 0.5,
            speaker_muted: false,
            // Vendor factory default for the capture endpoint (setVolumeScalarControl(-1, 0.92, "mic")).
            mic: 0.92,
            mic_muted: false,
        }
    }
}

/// Factory defaults for the four effect blocks (same defaults the original UI shows).
pub fn default_effects() -> [EffectState; 4] {
    [
        EffectState {
            enabled: false,
            level: 3,
            ..EffectState::default()
        },
        EffectState {
            enabled: false,
            level: 5,
            cutoff_hz: 200,
            ..EffectState::default()
        },
        EffectState {
            enabled: false,
            level: 0,
            ..EffectState::default()
        },
        EffectState {
            enabled: false,
            level: 3,
            ..EffectState::default()
        },
    ]
}

/// Full semantic audio snapshot rendered by the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioState {
    pub eq_enabled: bool,
    pub eq_bands: [i32; EQ_BANDS],
    pub effects: [EffectState; 4],
    pub surround: SurroundState,
    pub mic: MicState,
    pub volume: VolumeState,
    /// Surround amplification (`Enable_Channel_COPY_LFX`).
    pub surround_amplification: bool,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            eq_enabled: false,
            eq_bands: [0; EQ_BANDS],
            effects: default_effects(),
            surround: SurroundState::default(),
            mic: MicState::default(),
            volume: VolumeState::default(),
            surround_amplification: false,
        }
    }
}

impl AudioState {
    pub fn effect(&self, kind: EffectKind) -> &EffectState {
        &self.effects[kind as usize]
    }

    pub fn effect_mut(&mut self, kind: EffectKind) -> &mut EffectState {
        &mut self.effects[kind as usize]
    }
}

/// Capability flags describing what the active backend can actually do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub eq: bool,
    pub effects: bool,
    pub mic_dsp: bool,
    /// Microphone monitoring through the vendor sidecar (MicListen.dll).
    pub mic_listen: bool,
    pub surround: bool,
    pub volume: bool,
    pub mic_volume: bool,
    /// Test sounds can be played through the system mixer.
    pub test_sound: bool,
    /// HID status (battery/firmware) is available.
    pub hid_status: bool,
    /// HID commands (prompt volume / auto shutdown / reset) are available.
    pub hid_commands: bool,
    /// Device power-off / factory reset are wired.
    pub device_power: bool,
}

impl Capabilities {
    pub const NONE: Capabilities = Capabilities {
        eq: false,
        effects: false,
        mic_dsp: false,
        mic_listen: false,
        surround: false,
        volume: false,
        mic_volume: false,
        test_sound: false,
        hid_status: false,
        hid_commands: false,
        device_power: false,
    };

    pub fn all() -> Capabilities {
        Capabilities {
            eq: true,
            effects: true,
            mic_dsp: true,
        mic_listen: true,
            surround: true,
            volume: true,
            mic_volume: true,
            test_sound: true,
            hid_status: true,
            hid_commands: true,
            device_power: true,
        }
    }
}

/// A semantic change requested by the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioChange {
    EqEnabled(bool),
    EqBand { band: usize, value: i32 },
    EffectEnabled { kind: EffectKind, enabled: bool },
    EffectLevel { kind: EffectKind, level: i32 },
    BassCutoffHz(i32),
    AdaptiveMode(u8),
    VoiceNoiseSuppression(i32),
    MagicVoiceEnabled(bool),
    MagicVoiceType(u8),
    AiNoiseReduction(bool),
    NoiseSuppression(bool),
    SpeakerVolume(f32),
    SpeakerMute(bool),
    MicVolume(f32),
    MicMute(bool),
    SurroundEnabled(bool),
    SurroundMode(SurroundMode),
    SurroundRoom(RoomSize),
    SurroundAmplification(bool),
    MicListen(bool),
}

/// Speaker-test channels available in the original "7.1 test" workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestChannel {
    LeftFront,
    RightFront,
    Center,
    Subwoofer,
    LeftSurround,
    RightSurround,
    LeftBack,
    RightBack,
}

impl TestChannel {
    pub const ALL: [TestChannel; 8] = [
        TestChannel::LeftFront,
        TestChannel::RightFront,
        TestChannel::Center,
        TestChannel::Subwoofer,
        TestChannel::LeftSurround,
        TestChannel::RightSurround,
        TestChannel::LeftBack,
        TestChannel::RightBack,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            TestChannel::LeftFront => "test.lf",
            TestChannel::RightFront => "test.rf",
            TestChannel::Center => "test.c",
            TestChannel::Subwoofer => "test.sub",
            TestChannel::LeftSurround => "test.ls",
            TestChannel::RightSurround => "test.rs",
            TestChannel::LeftBack => "test.lb",
            TestChannel::RightBack => "test.rb",
        }
    }

    /// File name inside the original `libs\sound` directory.
    pub fn wav_name(self) -> &'static str {
        match self {
            TestChannel::LeftFront => "L-Front.wav",
            TestChannel::RightFront => "R-Front.wav",
            TestChannel::Center => "Center.wav",
            TestChannel::Subwoofer => "Subwoofer.wav",
            TestChannel::LeftSurround => "L-Surround.wav",
            TestChannel::RightSurround => "R-Surround.wav",
            TestChannel::LeftBack => "L-BackSurround.wav",
            TestChannel::RightBack => "R-BackSurround.wav",
        }
    }
}
