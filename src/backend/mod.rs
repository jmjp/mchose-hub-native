//! Backend abstraction: audio DSP control and device HID transport.
//!
//! Two independent planes are modelled after the original application:
//!
//! * [`AudioBackend`] — host-side audio effects (EQ, enhancement blocks, mic DSP,
//!   volumes, test sounds). The Windows implementation writes the C-Media driver
//!   property store; a mock backend is used by tests and in UI development.
//! * [`hid`] — the 0xAA HID protocol for device status and device commands.

pub mod hid;
pub mod mock;
pub mod protocol;

#[cfg(windows)]
pub mod confapi;
#[cfg(windows)]
pub mod driverinstall;
#[cfg(windows)]
pub mod miclisten;
#[cfg(windows)]
pub mod property_keys;
#[cfg(windows)]
pub mod winaudio;

#[cfg(not(windows))]
pub mod portable;

use crate::model::audio::{AudioChange, AudioState, Capabilities, TestChannel};

/// Errors surfaced by audio backends.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackendError {
    /// The requested control is not implemented by the active backend.
    #[error("control not supported by this backend: {0}")]
    Unsupported(&'static str),
    /// A Windows/COM call failed.
    #[error("windows audio error: {0}")]
    Windows(String),
    /// The property does not exist on the current endpoint/driver.
    #[error("property unavailable: {0}")]
    PropertyUnavailable(String),
    /// Underlying I/O failed.
    #[error("io error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, BackendError>;

/// Which implementation is active (shown in the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Windows property store backed by the C-Media driver (full DSP support).
    PropertyStore,
    /// Core Audio only (volume/mute; no DSP properties).
    CoreAudio,
    /// Portable system mixer backend (Linux/macOS).
    System,
    /// In-memory backend for development.
    Mock,
}

impl BackendKind {
    pub fn label_key(self) -> &'static str {
        match self {
            BackendKind::PropertyStore => "backend.property_store",
            BackendKind::CoreAudio => "backend.core_audio",
            BackendKind::System => "backend.system",
            BackendKind::Mock => "backend.mock",
        }
    }
}

/// Host-side audio DSP control.
pub trait AudioBackend {
    fn kind(&self) -> BackendKind;

    /// What this backend can read/write.
    fn capabilities(&self) -> Capabilities;

    /// Read the current audio state from the driver.
    fn read(&mut self) -> Result<AudioState>;

    /// Apply one semantic change.
    fn apply(&mut self, change: AudioChange) -> Result<()>;

    /// Play the channel identification sound.
    fn test_sound(&mut self, channel: TestChannel) -> Result<()>;

    /// Stop any currently playing test sound.
    fn stop_test_sound(&mut self) -> Result<()>;

    /// Re-read the state (e.g. after a device/driver change).
    fn reload(&mut self) -> Result<()> {
        self.read().map(|_| ())
    }

    /// Human-readable identifier of the active endpoint, when available.
    fn endpoint_name(&self) -> Option<String> {
        None
    }
}

/// Open the best available audio backend for this platform.
pub fn open_audio_backend() -> Box<dyn AudioBackend> {
    #[cfg(windows)]
    {
        match winaudio::WindowsAudioBackend::new() {
            Ok(backend) => {
                log::info!(
                    "audio backend: {:?} capabilities {:?}",
                    backend.kind(),
                    backend.capabilities()
                );
                return Box::new(backend);
            }
            Err(err) => {
                log::warn!("Windows audio backend unavailable ({err}); using mock backend");
            }
        }
    }
    #[cfg(not(windows))]
    {
        let backend = portable::PortableAudioBackend::new();
        log::info!(
            "audio backend: {:?} capabilities {:?}",
            backend.kind(),
            backend.capabilities()
        );
        return Box::new(backend);
    }
    #[cfg(not(windows))]
    #[allow(unreachable_code)]
    {
        log::warn!("no audio backend available; using in-memory mock");
        Box::new(mock::MockAudioBackend::new())
    }
    #[cfg(windows)]
    {
        log::warn!("using in-memory mock audio backend");
        Box::new(mock::MockAudioBackend::new())
    }
}

/// Open a backend for tests/UI development.
pub fn open_mock_backend() -> mock::MockAudioBackend {
    mock::MockAudioBackend::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_backend_supports_full_capabilities() {
        let mut backend = open_mock_backend();
        assert_eq!(backend.kind(), BackendKind::Mock);
        assert!(backend.capabilities().eq);
        let state = backend.read().expect("mock read");
        assert_eq!(state.eq_bands.len(), 10);
        backend
            .apply(AudioChange::EqEnabled(true))
            .expect("mock apply");
        assert!(backend.read().unwrap().eq_enabled);
    }
}
