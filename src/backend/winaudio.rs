//! Windows audio backend.
//!
//! Writes the C-Media driver's effect properties through the Windows property
//! store (`IMMDevice::OpenPropertyStore` → `IPropertyStore`), which is exactly
//! what the original `osConfLib.dll` (C-Media ConfLib) does internally:
//! `PropertyControl(name, ...)` hashes the vendor name to a `PROPERTYKEY` and
//! reads/writes it as a `VT_UI4` value (see `findings-native.md` §3).
//!
//! Endpoint volume/mute use Core Audio (`IAudioEndpointVolume`), matching the
//! original's `VolumeScalarControl`/`MuteControl` behaviour. Test sounds use the
//! vendor WAV files shipped in the original install (`libs\sound\*.wav`).

#![cfg(windows)]



use std::os::windows::ffi::OsStrExt;

use windows::core::{IUnknown, GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, PlaySoundW,
    SND_ASYNC, SND_FILENAME, SND_NODEFAULT,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READWRITE,
};
use windows::Win32::System::Variant::{VT_BOOL, VT_I4, VT_LPWSTR, VT_UI4};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

use crate::backend::confapi::ConfApi;
use crate::backend::property_keys::{self, names, PropertyKey};
use crate::backend::{AudioBackend, BackendError, BackendKind, Result};
use crate::config;
use crate::model::audio::{
    AudioChange, AudioState, Capabilities, EffectKind, RoomSize, SurroundMode, TestChannel,
};

/// Vendor key written by the original app before reading effect properties:
/// `PKEY` `{1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E}` pid 5 = 1 — forces the effects
/// chain to stay enabled while the app configures it.
const EFFECTS_CHAIN_KEY: PropertyKey = PropertyKey {
    guid: [
        0x03, 0xD8, 0xA5, 0x1D, 0x92, 0xD4, 0xDD, 0x4E, 0x8C, 0x23, 0xE0, 0xC0, 0xFF, 0xEE, 0x7F,
        0x0E,
    ],
    pid: 5,
};

/// Property name used to probe the C-Media render DSP.
const PROBE_RENDER: &str = names::ENABLE_EQ_GFX;
/// Property name used to probe the C-Media capture DSP.
const PROBE_CAPTURE: &str = names::AI_NR_ENABLE;

/// Windows property-store + Core Audio backend.
pub struct WindowsAudioBackend {
    render_store: Option<IPropertyStore>,
    capture_store: Option<IPropertyStore>,
    render_volume: Option<IAudioEndpointVolume>,
    capture_volume: Option<IAudioEndpointVolume>,
    caps: Capabilities,
    state: AudioState,
    endpoint_name: Option<String>,
    test_sound_playing: bool,
    /// Vendor C-Media ConfLib adapter (full DSP control) when the original app is installed.
    conf: Option<ConfApi>,
}

impl WindowsAudioBackend {
    pub fn new() -> Result<Self> {
        unsafe {
            // RPC_E_CHANGED_MODE is fine: COM may already be initialised as MTA.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(
                &MMDeviceEnumerator,
                None::<&IUnknown>,
                CLSCTX_ALL,
            )
            .map_err(|err| BackendError::Windows(format!("MMDeviceEnumerator: {err}")))?
        };

        let render_device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
            .map_err(|err| BackendError::Windows(format!("default render endpoint: {err}")))?;
        let capture_device =
            unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }.ok();

        let render_store = open_store(&render_device);
        let capture_store = capture_device.as_ref().and_then(open_store);
        let render_volume = activate_volume(&render_device);
        let capture_volume = capture_device.as_ref().and_then(activate_volume);
        let endpoint_name = render_store
            .as_ref()
            .and_then(read_friendly_name);

        let conf = ConfApi::load();
        if conf.is_none() {
            log::warn!(
                "C-Media ConfLib unavailable; DSP controls fall back to the endpoint property store"
            );
        }
        let mut backend = Self {
            render_store,
            capture_store,
            render_volume,
            capture_volume,
            caps: Capabilities::NONE,
            state: AudioState::default(),
            endpoint_name,
            test_sound_playing: false,
            conf,
        };

        // Force the C-Media effects chain on, as the vendor app does before reads.
        let _ = backend.set_u32_raw(EFFECTS_CHAIN_KEY, 1);
        backend.caps = backend.probe_capabilities();
        if let Err(err) = backend.read() {
            log::warn!("initial audio state read failed: {err}");
        }
        Ok(backend)
    }

    fn probe_capabilities(&self) -> Capabilities {
        let eq = self.get_u32_raw(PROBE_RENDER).is_ok();
        let mic_dsp = self.get_u32_raw(PROBE_CAPTURE).is_ok();
        Capabilities {
            eq,
            effects: eq,
            mic_dsp,
            mic_listen: crate::backend::miclisten::available(),
            surround: false, // vendor control handler, not a property-store value (yet)
            volume: self.render_volume.is_some(),
            mic_volume: self.capture_volume.is_some(),
            test_sound: true,
            hid_status: false,
            hid_commands: false,
            device_power: false,
        }
    }

    fn store_for(&self, name: &str) -> Option<&IPropertyStore> {
        if is_capture_property(name) {
            self.capture_store.as_ref().or(self.render_store.as_ref())
        } else {
            self.render_store.as_ref().or(self.capture_store.as_ref())
        }
    }

    fn get_u32_raw(&self, name: &str) -> Result<u32> {
        if let Some(conf) = self.conf.as_ref() {
            return conf.get(name).map(|value| value as u32);
        }
        let key = property_keys::lookup(name)
            .ok_or_else(|| BackendError::PropertyUnavailable(name.to_string()))?;
        let store = self
            .store_for(name)
            .ok_or_else(|| BackendError::PropertyUnavailable(name.to_string()))?;
        get_u32(store, key)
    }

    fn get_u32_key(&self, key: PropertyKey) -> Result<u32> {
        let store = self
            .render_store
            .as_ref()
            .ok_or_else(|| BackendError::PropertyUnavailable("render store".into()))?;
        get_u32(store, key)
    }

    fn set_u32_raw(&self, key: PropertyKey, value: u32) -> Result<()> {
        // The effects-chain key always goes to the render store; named properties
        // are routed by prefix.
        let store = self
            .render_store
            .as_ref()
            .ok_or(BackendError::PropertyUnavailable("render store".into()))?;
        set_u32(store, key, value)
    }

    fn set_named(&self, name: &str, value: u32) -> Result<()> {
        if let Some(conf) = self.conf.as_ref() {
            return conf.set(name, value as i32);
        }
        let key = property_keys::lookup(name)
            .ok_or_else(|| BackendError::PropertyUnavailable(name.to_string()))?;
        let store = self
            .store_for(name)
            .ok_or_else(|| BackendError::PropertyUnavailable(name.to_string()))?;
        set_u32(store, key, value)
    }

    fn set_bool(&self, name: &str, value: bool) -> Result<()> {
        self.set_named(name, u32::from(value))
    }

    fn read_volume(&mut self) {
        if let Some(volume) = self.render_volume.as_ref() {
            unsafe {
                if let Ok(level) = volume.GetMasterVolumeLevelScalar() {
                    self.state.volume.speaker = level;
                }
                if let Ok(muted) = volume.GetMute() {
                    self.state.volume.speaker_muted = muted.as_bool();
                }
            }
        }
        if let Some(volume) = self.capture_volume.as_ref() {
            unsafe {
                if let Ok(level) = volume.GetMasterVolumeLevelScalar() {
                    self.state.volume.mic = level;
                }
                if let Ok(muted) = volume.GetMute() {
                    self.state.volume.mic_muted = muted.as_bool();
                }
            }
        }
    }

    fn read_state(&mut self) -> Result<AudioState> {
        if self.caps.eq {
            if let Ok(enabled) = self.get_u32_raw(names::ENABLE_EQ_GFX) {
                self.state.eq_enabled = enabled != 0;
            }
            for band in 0..self.state.eq_bands.len() {
                let name = eq_band_name(band);
                if let Ok(value) = self.get_u32_raw(name) {
                    self.state.eq_bands[band] = decode_signed(value);
                }
            }
        }
        if self.caps.effects {
            let effect_names = [
                (
                    EffectKind::Brilliance,
                    names::ENABLE_AUDIOBRILLIANT_LFX,
                    names::AUDIOBRILLIANT_LEVEL,
                ),
                (
                    EffectKind::VirtualBass,
                    names::ENABLE_VIRTUALBASS_LFX,
                    names::VIRTUALBASS_LEVEL,
                ),
                (
                    EffectKind::AdaptiveVolume,
                    names::ENABLE_ADAPTIVEVOLUME_GFX,
                    names::ADAPTIVEVOLUME_LEVEL,
                ),
                (
                    EffectKind::VoiceClarity,
                    names::ENABLE_VOICECLARITY_LFX,
                    names::VOICECLARITY_LEVEL,
                ),
            ];
            for (kind, enable_name, level_name) in effect_names {
                if let Ok(value) = self.get_u32_raw(enable_name) {
                    self.state.effect_mut(kind).enabled = value != 0;
                }
                if let Ok(value) = self.get_u32_raw(level_name) {
                    self.state.effect_mut(kind).level = decode_signed(value);
                }
            }
            if let Ok(value) = self.get_u32_raw(names::VIRTUALBASS_CUTOFFFREQUENCY) {
                self.state.effect_mut(EffectKind::VirtualBass).cutoff_hz = decode_signed(value);
            }
            if let Ok(value) = self.get_u32_raw(names::ADAPTIVEVOLUME_MODE) {
                self.state.effect_mut(EffectKind::AdaptiveVolume).mode = value as u8;
            }
            if let Ok(value) = self.get_u32_raw(names::VOICECLARITY_NOISESUPP_LEVEL) {
                self.state.effect_mut(EffectKind::VoiceClarity).noise_suppression =
                    decode_signed(value);
            }
            if let Ok(value) = self.get_u32_raw(names::ENABLE_CHANNEL_COPY_LFX) {
                self.state.surround_amplification = value != 0;
            }
        }
        if self.caps.mic_dsp {
            if let Ok(value) = self.get_u32_raw(names::AI_NR_ENABLE) {
                self.state.mic.ai_noise_reduction = value != 0;
            }
            if let Ok(value) = self.get_u32_raw(names::NR_ENABLE) {
                self.state.mic.noise_suppression = value != 0;
            }
            if let Ok(value) = self.get_u32_raw(names::ENABLE_MAGICVOICE) {
                self.state.mic.magic_voice = value != 0;
            }
            if let Ok(value) = self.get_u32_raw(names::MAGICVOICE_SELECTION) {
                self.state.mic.magic_voice_type = value as u8;
            }
        }
        self.read_volume();
        Ok(self.state.clone())
    }
}

impl AudioBackend for WindowsAudioBackend {
    fn kind(&self) -> BackendKind {
        if self.conf.is_some() || self.caps.eq || self.caps.effects || self.caps.mic_dsp {
            BackendKind::PropertyStore
        } else {
            BackendKind::CoreAudio
        }
    }

    fn capabilities(&self) -> Capabilities {
        self.caps
    }

    fn read(&mut self) -> Result<AudioState> {
        self.read_state()
    }

    fn apply(&mut self, change: AudioChange) -> Result<()> {
        match change {
            AudioChange::EqEnabled(enabled) => {
                if !self.caps.eq {
                    return Err(BackendError::Unsupported("equalizer"));
                }
                self.set_bool(names::ENABLE_EQ_GFX, enabled)?;
                self.state.eq_enabled = enabled;
            }
            AudioChange::EqBand { band, value } => {
                if !self.caps.eq || band >= self.state.eq_bands.len() {
                    return Err(BackendError::Unsupported("equalizer band"));
                }
                let value = value.clamp(
                    crate::model::audio::EQ_GAIN_MIN,
                    crate::model::audio::EQ_GAIN_MAX,
                );
                self.set_named(eq_band_name(band), encode_signed(value))?;
                self.state.eq_bands[band] = value;
            }
            AudioChange::EffectEnabled { kind, enabled } => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("sound effects"));
                }
                self.set_bool(effect_enable_name(kind), enabled)?;
                self.state.effect_mut(kind).enabled = enabled;
            }
            AudioChange::EffectLevel { kind, level } => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("sound effects"));
                }
                let level = clamp_effect_level(kind, level);
                self.set_named(effect_level_name(kind), encode_signed(level))?;
                self.state.effect_mut(kind).level = level;
            }
            AudioChange::BassCutoffHz(hz) => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("bass cutoff"));
                }
                let hz = hz.clamp(20, 20_000);
                self.set_named(names::VIRTUALBASS_CUTOFFFREQUENCY, encode_signed(hz))?;
                self.state.effect_mut(EffectKind::VirtualBass).cutoff_hz = hz;
            }
            AudioChange::AdaptiveMode(mode) => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("adaptive volume mode"));
                }
                let mode = mode.min(1);
                self.set_named(names::ADAPTIVEVOLUME_MODE, u32::from(mode))?;
                self.state.effect_mut(EffectKind::AdaptiveVolume).mode = mode;
            }
            AudioChange::VoiceNoiseSuppression(value) => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("noise suppression"));
                }
                let value = value.clamp(0, 20);
                self.set_named(names::VOICECLARITY_NOISESUPP_LEVEL, encode_signed(value))?;
                self.state
                    .effect_mut(EffectKind::VoiceClarity)
                    .noise_suppression = value;
            }
            AudioChange::MagicVoiceEnabled(enabled) => {
                if !self.caps.mic_dsp {
                    return Err(BackendError::Unsupported("magic voice"));
                }
                self.set_bool(names::ENABLE_MAGICVOICE, enabled)?;
                self.state.mic.magic_voice = enabled;
            }
            AudioChange::MagicVoiceType(kind) => {
                if !self.caps.mic_dsp {
                    return Err(BackendError::Unsupported("magic voice"));
                }
                let kind = kind.min(3);
                self.set_named(names::MAGICVOICE_SELECTION, u32::from(kind))?;
                self.state.mic.magic_voice_type = kind;
            }
            AudioChange::AiNoiseReduction(enabled) => {
                if !self.caps.mic_dsp {
                    return Err(BackendError::Unsupported("AI noise reduction"));
                }
                self.set_bool(names::AI_NR_ENABLE, enabled)?;
                self.state.mic.ai_noise_reduction = enabled;
            }
            AudioChange::NoiseSuppression(enabled) => {
                if !self.caps.mic_dsp {
                    return Err(BackendError::Unsupported("noise suppression"));
                }
                self.set_bool(names::NR_ENABLE, enabled)?;
                self.state.mic.noise_suppression = enabled;
            }
            AudioChange::SpeakerVolume(value) => {
                let volume = self
                    .render_volume
                    .as_ref()
                    .ok_or(BackendError::Unsupported("speaker volume"))?;
                let value = value.clamp(0.0, 1.0);
                unsafe {
                    volume
                        .SetMasterVolumeLevelScalar(value, std::ptr::null())
                        .map_err(|err| BackendError::Windows(err.to_string()))?;
                }
                self.state.volume.speaker = value;
            }
            AudioChange::SpeakerMute(muted) => {
                let volume = self
                    .render_volume
                    .as_ref()
                    .ok_or(BackendError::Unsupported("speaker mute"))?;
                unsafe {
                    volume
                        .SetMute(muted, std::ptr::null())
                        .map_err(|err| BackendError::Windows(err.to_string()))?;
                }
                self.state.volume.speaker_muted = muted;
            }
            AudioChange::MicVolume(value) => {
                let volume = self
                    .capture_volume
                    .as_ref()
                    .ok_or(BackendError::Unsupported("microphone volume"))?;
                let value = value.clamp(0.0, 1.0);
                unsafe {
                    volume
                        .SetMasterVolumeLevelScalar(value, std::ptr::null())
                        .map_err(|err| BackendError::Windows(err.to_string()))?;
                }
                self.state.volume.mic = value;
            }
            AudioChange::MicMute(muted) => {
                let volume = self
                    .capture_volume
                    .as_ref()
                    .ok_or(BackendError::Unsupported("microphone mute"))?;
                unsafe {
                    volume
                        .SetMute(muted, std::ptr::null())
                        .map_err(|err| BackendError::Windows(err.to_string()))?;
                }
                self.state.volume.mic_muted = muted;
            }
            AudioChange::SurroundAmplification(enabled) => {
                if !self.caps.effects {
                    return Err(BackendError::Unsupported("surround amplification"));
                }
                self.set_bool(names::ENABLE_CHANNEL_COPY_LFX, enabled)?;
                self.state.surround_amplification = enabled;
            }
            AudioChange::SurroundEnabled(_)
            | AudioChange::SurroundMode(_)
            | AudioChange::SurroundRoom(_) => {
                return Err(BackendError::Unsupported(
                    "virtual surround (vendor C-Media control handler)",
                ));
            }
            AudioChange::MicListen(enabled) => {
                crate::backend::miclisten::set_listen(enabled, None)
                    .map_err(BackendError::Io)?;
                self.state.mic.mic_listen = enabled;
            }
        }
        Ok(())
    }

    fn test_sound(&mut self, channel: TestChannel) -> Result<()> {
        let Some(path) = config::find_test_sound(channel.wav_name()) else {
            return Err(BackendError::Io(format!(
                "test sound {} not found (original MCHOSE HUB install is required)",
                channel.wav_name()
            )));
        };
        play_wav(&path).map_err(BackendError::Io)?;
        self.test_sound_playing = true;
        Ok(())
    }

    fn stop_test_sound(&mut self) -> Result<()> {
        unsafe {
            let _ = PlaySoundW(PCWSTR::null(), None, SND_ASYNC);
        }
        self.test_sound_playing = false;
        Ok(())
    }

    fn endpoint_name(&self) -> Option<String> {
        self.endpoint_name.clone()
    }
}

fn play_wav(path: &std::path::Path) -> std::result::Result<(), String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let ok = PlaySoundW(
            PCWSTR(wide.as_ptr()),
            None,
            SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
        );
        if ok.as_bool() {
            Ok(())
        } else {
            Err(format!("PlaySoundW failed for {}", path.display()))
        }
    }
}

fn open_store(device: &IMMDevice) -> Option<IPropertyStore> {
    unsafe { device.OpenPropertyStore(STGM_READWRITE).ok() }
}

fn activate_volume(device: &IMMDevice) -> Option<IAudioEndpointVolume> {
    unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok() }
}

fn to_propertykey(key: PropertyKey) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID {
            data1: u32::from_le_bytes([key.guid[0], key.guid[1], key.guid[2], key.guid[3]]),
            data2: u16::from_le_bytes([key.guid[4], key.guid[5]]),
            data3: u16::from_le_bytes([key.guid[6], key.guid[7]]),
            data4: [
                key.guid[8], key.guid[9], key.guid[10], key.guid[11], key.guid[12], key.guid[13],
                key.guid[14], key.guid[15],
            ],
        },
        pid: key.pid,
    }
}

fn get_u32(store: &IPropertyStore, key: PropertyKey) -> Result<u32> {
    let pk = to_propertykey(key);
    let mut prop = unsafe { store.GetValue(&pk) }
        .map_err(|err| BackendError::Windows(format!("GetValue: {err}")))?;
    let value = decode_variant(&prop);
    unsafe {
        let _ = PropVariantClear(&mut prop);
    }
    value.ok_or_else(|| BackendError::PropertyUnavailable(format!("property {key:?}")))
}

fn set_u32(store: &IPropertyStore, key: PropertyKey, value: u32) -> Result<()> {
    let pk = to_propertykey(key);
    let mut prop = encode_variant_u32(value);
    let result = unsafe { store.SetValue(&pk, &prop) };
    unsafe {
        let _ = PropVariantClear(&mut prop);
    }
    result.map_err(|err| BackendError::Windows(format!("SetValue: {err}")))
}

fn decode_variant(prop: &PROPVARIANT) -> Option<u32> {
    unsafe {
        let inner = &*prop.Anonymous.Anonymous;
        if inner.vt == VT_UI4 {
            Some(inner.Anonymous.ulVal)
        } else if inner.vt == VT_I4 {
            Some(inner.Anonymous.lVal as u32)
        } else if inner.vt == VT_BOOL {
            Some(u32::from(inner.Anonymous.boolVal.as_bool()))
        } else {
            None
        }
    }
}

fn encode_variant_u32(value: u32) -> PROPVARIANT {
    let mut prop = PROPVARIANT::default();
    unsafe {
        let inner = &mut *prop.Anonymous.Anonymous;
        inner.vt = VT_UI4;
        inner.Anonymous.ulVal = value;
    }
    prop
}

fn _encode_variant_string_placeholder() {
    // VT_LPWSTR is reserved for string-valued endpoint properties (not written yet).
    let _ = VT_LPWSTR;
}

fn read_friendly_name(store: &IPropertyStore) -> Option<String> {
    // PKEY_Device_FriendlyName = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid 14.
    let key = PropertyKey {
        guid: [
            0x4E, 0x25, 0x5C, 0xA4, 0x1C, 0xDF, 0xFD, 0x4E, 0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8,
            0x50, 0xE0,
        ],
        pid: 14,
    };
    let pk = to_propertykey(key);
    let mut prop = unsafe { store.GetValue(&pk) }.ok()?;
    let name = unsafe {
        let inner = &*prop.Anonymous.Anonymous;
        if inner.vt == VT_LPWSTR {
            let ptr: PWSTR = inner.Anonymous.pwszVal;
            if ptr.is_null() {
                None
            } else {
                Some(ptr.to_string().ok()?)
            }
        } else {
            None
        }
    };
    unsafe {
        let _ = PropVariantClear(&mut prop);
    }
    name
}

fn is_capture_property(name: &str) -> bool {
    name.starts_with("AI_")
        || name.starts_with("NR_")
        || name.starts_with("AEC_")
        || name.starts_with("Mic")
        || name.starts_with("MIC")
        || name.contains("MAGICVOICE")
        || name.starts_with("ENC_")
        || name.starts_with("NoiseReduction")
        || name.starts_with("Enable_NOISEREDUCTION")
        || name.starts_with("Enable_MIC")
}

fn eq_band_name(band: usize) -> &'static str {
    match band {
        0 => names::EQ_SLIDER1,
        1 => names::EQ_SLIDER2,
        2 => names::EQ_SLIDER3,
        3 => names::EQ_SLIDER4,
        4 => names::EQ_SLIDER5,
        5 => names::EQ_SLIDER6,
        6 => names::EQ_SLIDER7,
        7 => names::EQ_SLIDER8,
        8 => names::EQ_SLIDER9,
        _ => names::EQ_SLIDER10,
    }
}

fn effect_enable_name(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Brilliance => names::ENABLE_AUDIOBRILLIANT_LFX,
        EffectKind::VirtualBass => names::ENABLE_VIRTUALBASS_LFX,
        EffectKind::AdaptiveVolume => names::ENABLE_ADAPTIVEVOLUME_GFX,
        EffectKind::VoiceClarity => names::ENABLE_VOICECLARITY_LFX,
    }
}

fn effect_level_name(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Brilliance => names::AUDIOBRILLIANT_LEVEL,
        EffectKind::VirtualBass => names::VIRTUALBASS_LEVEL,
        EffectKind::AdaptiveVolume => names::ADAPTIVEVOLUME_LEVEL,
        EffectKind::VoiceClarity => names::VOICECLARITY_LEVEL,
    }
}

/// Signed 32-bit value round-tripped through the driver as `u32`.
fn encode_signed(value: i32) -> u32 {
    value as u32
}

fn decode_signed(value: u32) -> i32 {
    value as i32
}

fn clamp_effect_level(kind: EffectKind, level: i32) -> i32 {
    match kind {
        EffectKind::Brilliance | EffectKind::VirtualBass | EffectKind::VoiceClarity => {
            level.clamp(0, 10)
        }
        EffectKind::AdaptiveVolume => level.clamp(0, 100),
    }
}


/// Diagnostic helper for `--probe`: try the alternative ways of obtaining the
/// C-Media effect property store and report what each one exposes.
pub fn diagnose_stores() -> serde_json::Value {
    use serde_json::json;
    let result = std::panic::catch_unwind(|| -> serde_json::Value {
        let mut out = serde_json::Map::new();
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let enumerator: IMMDeviceEnumerator =
                match CoCreateInstance(&MMDeviceEnumerator, None::<&IUnknown>, CLSCTX_ALL) {
                    Ok(value) => value,
                    Err(err) => return json!({ "enumerator_error": err.to_string() }),
                };
            let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                Ok(value) => value,
                Err(err) => return json!({ "device_error": err.to_string() }),
            };
            // Route 1: OpenPropertyStore (device/endpoint store).
            if let Ok(store) = device.OpenPropertyStore(STGM_READWRITE) {
                out.insert("open_store".into(), inspect_store(&store));
            } else {
                out.insert("open_store_error".into(), json!(true));
            }
            // Route 2: Activate<IPropertyStore> (effect/FX property store).
            match device.Activate::<IPropertyStore>(CLSCTX_ALL, None) {
                Ok(store) => {
                    out.insert("activated_store".into(), inspect_store(&store));
                }
                Err(err) => {
                    out.insert("activated_store_error".into(), json!(err.to_string()));
                }
            }
        }
        serde_json::Value::Object(out)
    });
    result.unwrap_or_else(|_| json!({ "panic": true }))
}

fn inspect_store(store: &IPropertyStore) -> serde_json::Value {
    use serde_json::json;
    unsafe {
        let count = store.GetCount().unwrap_or(0);
        let mut keys = Vec::new();
        for index in 0..count.min(4000) {
            let mut key = windows::Win32::Foundation::PROPERTYKEY::default();
            if store.GetAt(index, &mut key).is_ok() {
                keys.push(format!("{:?}/{}", key.fmtid, key.pid));
            }
        }
        let probe = property_keys::lookup(names::ENABLE_EQ_GFX)
            .map(|key| get_u32(store, key).ok());
        json!({ "key_count": count, "first_keys": keys.iter().take(40).cloned().collect::<Vec<_>>(), "enable_eq_gfx": probe })
    }
}

/// Unused helper kept for the surround page once the vendor mapping is known.
#[allow(dead_code)]
fn surround_defaults() -> (SurroundMode, RoomSize) {
    (SurroundMode::Music, RoomSize::Medium)
}
