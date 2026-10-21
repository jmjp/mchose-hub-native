//! Portable audio backend for Linux/macOS.
//!
//! The vendor DSP (C-Media property store, equalizer, effects and microphone
//! processing) is Windows-only: it lives in the C-Media ConfLib/APO components
//! shipped with the official MCHOSE HUB. On other platforms this backend exposes
//! the system output/input volume and mute state through the platform's own mixer
//! (PipeWire/PulseAudio on Linux, CoreAudio via `osascript` on macOS) and reports
//! the DSP capabilities as unavailable.
//!
//! * Linux: `wpctl` (PipeWire) first, falling back to `pactl` (PulseAudio).
//! * macOS: `osascript` output-volume APIs (input volume is not exposed).
//!
//! HID communication still works on these platforms through `hidapi` (with the
//! usual udev rules on Linux and Input Monitoring permission on macOS).

use std::process::Command;

use crate::backend::{AudioBackend, BackendError, BackendKind, Result};
use crate::model::audio::{AudioChange, AudioState, Capabilities, TestChannel};

/// Audio backend driven by the operating system mixer.
pub struct PortableAudioBackend {
    state: AudioState,
    caps: Capabilities,
    endpoint: Option<String>,
}

impl PortableAudioBackend {
    pub fn new() -> Self {
        let mut caps = Capabilities::NONE;
        #[cfg(target_os = "linux")]
        {
            caps.volume = mixer_available();
            caps.mic_volume = caps.volume;
        }
        #[cfg(target_os = "macos")]
        {
            caps.volume = osascript_available();
        }
        let endpoint = endpoint_name();
        Self {
            state: AudioState::default(),
            caps,
            endpoint,
        }
    }
}

impl Default for PortableAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for PortableAudioBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::System
    }

    fn capabilities(&self) -> Capabilities {
        self.caps
    }

    fn read(&mut self) -> Result<AudioState> {
        if self.caps.volume {
            if let Some((volume, muted)) = read_output() {
                self.state.volume.speaker = volume;
                self.state.volume.speaker_muted = muted;
            }
        }
        if self.caps.mic_volume {
            if let Some((volume, muted)) = read_input() {
                self.state.volume.mic = volume;
                self.state.volume.mic_muted = muted;
            }
        }
        Ok(self.state)
    }

    fn apply(&mut self, change: AudioChange) -> Result<()> {
        match change {
            AudioChange::SpeakerVolume(value) => {
                if !self.caps.volume {
                    return Err(BackendError::Unsupported("system volume"));
                }
                set_output_volume(value)?;
                self.state.volume.speaker = value;
            }
            AudioChange::SpeakerMute(muted) => {
                if !self.caps.volume {
                    return Err(BackendError::Unsupported("system mute"));
                }
                set_output_mute(muted)?;
                self.state.volume.speaker_muted = muted;
            }
            AudioChange::MicVolume(value) => {
                if !self.caps.mic_volume {
                    return Err(BackendError::Unsupported("microphone volume"));
                }
                set_input_volume(value)?;
                self.state.volume.mic = value;
            }
            AudioChange::MicMute(muted) => {
                if !self.caps.mic_volume {
                    return Err(BackendError::Unsupported("microphone mute"));
                }
                set_input_mute(muted)?;
                self.state.volume.mic_muted = muted;
            }
            other => {
                return Err(BackendError::Unsupported(match other {
                    AudioChange::EqEnabled(_) | AudioChange::EqBand { .. } => "equalizer",
                    AudioChange::EffectEnabled { .. } | AudioChange::EffectLevel { .. } => {
                        "sound effects"
                    }
                    AudioChange::BassCutoffHz(_) => "bass cutoff",
                    AudioChange::AdaptiveMode(_) => "adaptive volume mode",
                    AudioChange::VoiceNoiseSuppression(_) => "noise suppression",
                    AudioChange::SurroundEnabled(_)
                    | AudioChange::SurroundMode(_)
                    | AudioChange::SurroundRoom(_)
                    | AudioChange::SurroundAmplification(_) => "virtual surround",
                    AudioChange::AiNoiseReduction(_)
                    | AudioChange::NoiseSuppression(_)
                    | AudioChange::MagicVoiceEnabled(_)
                    | AudioChange::MagicVoiceType(_)
                    | AudioChange::MicListen(_) => "microphone DSP",
                    _ => "audio control",
                }))
            }
        }
        Ok(())
    }

    fn test_sound(&mut self, _channel: TestChannel) -> Result<()> {
        Err(BackendError::Unsupported(
            "channel test sounds require the vendor audio assets",
        ))
    }

    fn stop_test_sound(&mut self) -> Result<()> {
        Ok(())
    }

    fn endpoint_name(&self) -> Option<String> {
        self.endpoint.clone()
    }
}

fn endpoint_name() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        if command_ok("wpctl", &["--version"]) {
            return Some("PipeWire default output".to_string());
        }
        if command_ok("pactl", &["--version"]) {
            return Some("PulseAudio default output".to_string());
        }
    }
    #[cfg(target_os = "macos")]
    {
        if osascript_available() {
            return Some("macOS system output".to_string());
        }
    }
    None
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn mixer_available() -> bool {
    command_ok("wpctl", &["--version"]) || command_ok("pactl", &["--version"])
}

#[cfg(target_os = "linux")]
fn read_output() -> Option<(f32, bool)> {
    if let Ok(output) = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some((volume, muted)) = parse_wpctl(&text) {
                return Some((volume, muted));
            }
        }
    }
    read_pactl(&["get-sink-volume", "@DEFAULT_SINK@"], &["get-sink-mute", "@DEFAULT_SINK@"])
}

#[cfg(target_os = "linux")]
fn read_input() -> Option<(f32, bool)> {
    if let Ok(output) = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some((volume, muted)) = parse_wpctl(&text) {
                return Some((volume, muted));
            }
        }
    }
    read_pactl(
        &["get-source-volume", "@DEFAULT_SOURCE@"],
        &["get-source-mute", "@DEFAULT_SOURCE@"],
    )
}

#[cfg(target_os = "linux")]
fn parse_wpctl(text: &str) -> Option<(f32, bool)> {
    // "Volume: 0.91" or "Volume: 0.91 [MUTED]"
    let volume = text.split("Volume:").nth(1)?;
    let value = volume
        .split_whitespace()
        .next()?
        .trim()
        .parse::<f32>()
        .ok()?;
    Some((value.clamp(0.0, 1.0), text.contains("[MUTED]")))
}

#[cfg(target_os = "linux")]
fn read_pactl(volume_args: &[&str], mute_args: &[&str]) -> Option<(f32, bool)> {
    let volume_output = Command::new("pactl").args(volume_args).output().ok()?;
    if !volume_output.status.success() {
        return None;
    }
    let volume_text = String::from_utf8_lossy(&volume_output.stdout);
    // "Volume: front-left: 52428 /  80% / -5.81 dB, ..."
    let percent = volume_text
        .split('%')
        .next()?
        .rsplit('/')
        .next()?
        .trim()
        .parse::<f32>()
        .ok()?;
    let mute_output = Command::new("pactl").args(mute_args).output().ok()?;
    let muted = String::from_utf8_lossy(&mute_output.stdout).contains("yes");
    Some(((percent / 100.0).clamp(0.0, 1.0), muted))
}

#[cfg(target_os = "linux")]
fn set_output_volume(value: f32) -> Result<()> {
    set_linux_volume("@DEFAULT_AUDIO_SINK@", "@DEFAULT_SINK@", value, false)
}

#[cfg(target_os = "linux")]
fn set_output_mute(muted: bool) -> Result<()> {
    set_linux_mute("@DEFAULT_AUDIO_SINK@", "@DEFAULT_SINK@", muted, false)
}

#[cfg(target_os = "linux")]
fn set_input_volume(value: f32) -> Result<()> {
    set_linux_volume("@DEFAULT_AUDIO_SOURCE@", "@DEFAULT_SOURCE@", value, true)
}

#[cfg(target_os = "linux")]
fn set_input_mute(muted: bool) -> Result<()> {
    set_linux_mute(
        "@DEFAULT_AUDIO_SOURCE@",
        "@DEFAULT_SOURCE@",
        muted,
        true,
    )
}

#[cfg(target_os = "linux")]
fn set_linux_volume(wpctl_target: &str, pactl_target: &str, value: f32, input: bool) -> Result<()> {
    let percent = (value.clamp(0.0, 1.0) * 100.0).round() as u32;
    let argument = format!("{percent}%");
    if command_ok("wpctl", &["--version"]) {
        let status = Command::new("wpctl")
            .args(["set-volume", wpctl_target, &argument])
            .status()
            .map_err(|err| BackendError::Io(err.to_string()))?;
        if status.success() {
            return Ok(());
        }
    }
    if command_ok("pactl", &["--version"]) {
        let (verb, target) = if input {
            ("set-source-volume", pactl_target)
        } else {
            ("set-sink-volume", pactl_target)
        };
        let status = Command::new("pactl")
            .args([verb, target, &argument])
            .status()
            .map_err(|err| BackendError::Io(err.to_string()))?;
        if status.success() {
            return Ok(());
        }
    }
    Err(BackendError::Io("no working system mixer found".into()))
}

#[cfg(target_os = "linux")]
fn set_linux_mute(wpctl_target: &str, pactl_target: &str, muted: bool, input: bool) -> Result<()> {
    let flag = if muted { "1" } else { "0" };
    if command_ok("wpctl", &["--version"]) {
        let status = Command::new("wpctl")
            .args(["set-mute", wpctl_target, flag])
            .status()
            .map_err(|err| BackendError::Io(err.to_string()))?;
        if status.success() {
            return Ok(());
        }
    }
    if command_ok("pactl", &["--version"]) {
        let (verb, target) = if input {
            ("set-source-mute", pactl_target)
        } else {
            ("set-sink-mute", pactl_target)
        };
        let status = Command::new("pactl")
            .args([verb, target, flag])
            .status()
            .map_err(|err| BackendError::Io(err.to_string()))?;
        if status.success() {
            return Ok(());
        }
    }
    Err(BackendError::Io("no working system mixer found".into()))
}

#[cfg(target_os = "macos")]
fn osascript_available() -> bool {
    command_ok("osascript", &["-e", "return 0"])
}

#[cfg(target_os = "macos")]
fn read_output() -> Option<(f32, bool)> {
    let output = Command::new("osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let volume = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f32>()
        .ok()?;
    let muted_output = Command::new("osascript")
        .args(["-e", "output muted of (get volume settings)"])
        .output()
        .ok()?;
    let muted = String::from_utf8_lossy(&muted_output.stdout)
        .trim()
        .eq_ignore_ascii_case("true");
    Some(((volume / 100.0).clamp(0.0, 1.0), muted))
}

#[cfg(target_os = "macos")]
fn read_input() -> Option<(f32, bool)> {
    None
}

#[cfg(target_os = "macos")]
fn set_output_volume(value: f32) -> Result<()> {
    let percent = (value.clamp(0.0, 1.0) * 100.0).round() as u32;
    let script = format!("set volume output volume {percent}");
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|err| BackendError::Io(err.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(BackendError::Io("osascript set volume failed".into()))
    }
}

#[cfg(target_os = "macos")]
fn set_output_mute(muted: bool) -> Result<()> {
    let script = format!("set volume output muted {}", if muted { "true" } else { "false" });
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|err| BackendError::Io(err.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(BackendError::Io("osascript set mute failed".into()))
    }
}

#[cfg(target_os = "macos")]
fn set_input_volume(_value: f32) -> Result<()> {
    Err(BackendError::Unsupported("microphone volume on macOS"))
}

#[cfg(target_os = "macos")]
fn set_input_mute(_muted: bool) -> Result<()> {
    Err(BackendError::Unsupported("microphone mute on macOS"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_output() -> Option<(f32, bool)> {
    None
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_input() -> Option<(f32, bool)> {
    None
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn set_output_volume(_value: f32) -> Result<()> {
    Err(BackendError::Unsupported("system volume"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn set_output_mute(_muted: bool) -> Result<()> {
    Err(BackendError::Unsupported("system mute"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn set_input_volume(_value: f32) -> Result<()> {
    Err(BackendError::Unsupported("microphone volume"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn set_input_mute(_muted: bool) -> Result<()> {
    Err(BackendError::Unsupported("microphone mute"))
}
