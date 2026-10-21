//! Domain model: device identity/status, semantic audio state and EQ presets.
//!
//! Submodules are the canonical import paths; only the handful of names used
//! across several layers are re-exported here.

pub mod audio;
pub mod device;
pub mod presets;

pub use audio::EffectKind;
pub use device::HidDeviceDescriptor;
pub use presets::{EqPreset, LocalizedNames};
