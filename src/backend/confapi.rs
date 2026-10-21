//! Native adapter for the vendor C-Media **ConfLib** (`osConfLib.dll`).
//!
//! This is the audio configuration engine the original MCHOSE HUB uses (loaded by
//! koffi in its AudioSDK worker). The ABI was recovered from the shipped DLL and
//! its decompilation (`mchose-hub-re/captures/native/ghidra/osconf/`):
//!
//! ```text
//! int ConfLibInit(void);
//! int CreateDeviceList(void);
//! int GetDeviceCount(uint type /*0=render,1=capture*/, uint* out);
//! int GetDeviceById(uint type, int index, DEVICEINFO* out, uint64 extra);
//! int PropertyControl(DEVICEINFO* dev, wchar_t* name, void** buf, uint64 unused, uchar action);
//! int ConfLibUnInit(void);
//!
//! DEVICEINFO = { int32 id; int32 status; int32 type; }
//! ```
//!
//! `action == 0` is GET: `*buf` must point to a 0x400-byte buffer which receives a
//! NUL-terminated wide decimal string. Any non-zero action is SET: `*buf` must point
//! to a `u32` value. The vendor property names are the same ones the renderer uses
//! (`Enable_EQ_GFX`, `EQ_Slider1..10`, `VIRTUALBASS_Level`, …).
//!
//! Loading the DLL directly in a normal process works (verified: `ConfLibInit` and
//! `CreateDeviceList` return 0, one render + one capture device are enumerated, and
//! GET/SET round-trip), so full DSP parity is possible without reproducing the
//! vendor's elevated SDK host.

use std::ffi::c_void;
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::backend::{BackendError, Result};

/// The 12-byte device record returned by `GetDeviceById`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceInfo {
    pub id: i32,
    pub status: i32,
    pub kind: i32,
}

type FnInit = unsafe extern "system" fn() -> i32;
type FnCreateDeviceList = unsafe extern "system" fn() -> i32;
type FnGetDeviceById = unsafe extern "system" fn(u32, i32, *mut DeviceInfo, u64) -> i32;
type FnPropertyControl =
    unsafe extern "system" fn(*mut DeviceInfo, *const u16, *mut *mut c_void, u64, u8) -> i32;
type FnUnInit = unsafe extern "system" fn() -> i32;

/// Loaded ConfLib instance bound to one render and one capture device.
pub struct ConfApi {
    _library: Library,
    init: FnInit,
    uninit: FnUnInit,
    get_by_id: FnGetDeviceById,
    property_control: FnPropertyControl,
    render: DeviceInfo,
    capture: Option<DeviceInfo>,
    path: PathBuf,
}

impl std::fmt::Debug for ConfApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfApi")
            .field("path", &self.path)
            .field("render", &self.render)
            .field("capture", &self.capture)
            .finish()
    }
}

impl ConfApi {
    /// Locate and initialise the vendor ConfLib.
    ///
    /// Returns `None` when the original MCHOSE HUB (and thus the C-Media ConfLib)
    /// is not installed, or when no render device can be enumerated.
    pub fn load() -> Option<Self> {
        let path = crate::config::find_conf_lib()?;
        match Self::load_from(&path) {
            Ok(api) => {
                log::info!("C-Media ConfLib loaded from {}", path.display());
                Some(api)
            }
            Err(err) => {
                log::warn!("C-Media ConfLib found at {} but unusable: {err}", path.display());
                None
            }
        }
    }

    fn load_from(path: &Path) -> Result<Self> {
        // SAFETY: the DLL exports the documented ConfLib C ABI; the library handle is
        // kept alive for the whole lifetime of this object.
        unsafe {
            let library = Library::new(path)
                .map_err(|err| BackendError::Io(format!("osConfLib load: {err}")))?;

            macro_rules! symbol {
                ($name:literal, $ty:ty) => {{
                    let sym: libloading::Symbol<$ty> = library.get($name).map_err(|err| {
                        BackendError::Io(format!(
                            "osConfLib export {} missing: {err}",
                            String::from_utf8_lossy($name)
                        ))
                    })?;
                    *sym
                }};
            }

            let init: FnInit = symbol!(b"ConfLibInit", FnInit);
            let create_device_list: FnCreateDeviceList =
                symbol!(b"CreateDeviceList", FnCreateDeviceList);
            let get_by_id: FnGetDeviceById = symbol!(b"GetDeviceById", FnGetDeviceById);
            let property_control: FnPropertyControl =
                symbol!(b"PropertyControl", FnPropertyControl);
            let uninit: FnUnInit = symbol!(b"ConfLibUnInit", FnUnInit);

            if init() != 0 {
                return Err(BackendError::Io("ConfLibInit failed".into()));
            }
            if create_device_list() != 0 {
                let _ = uninit();
                return Err(BackendError::Io("CreateDeviceList failed".into()));
            }

            let mut render = DeviceInfo::default();
            if get_by_id(0, 0, &mut render, 0) != 0 {
                let _ = uninit();
                return Err(BackendError::Io("no render device in ConfLib".into()));
            }
            let mut capture = DeviceInfo::default();
            let capture = if get_by_id(1, 0, &mut capture, 0) == 0 {
                Some(capture)
            } else {
                None
            };

            Ok(Self {
                _library: library,
                init,
                uninit,
                get_by_id,
                property_control,
                render,
                capture,
                path: path.to_path_buf(),
            })
        }
    }

    /// Re-enumerate the device list (e.g. after a device re-plug).
    pub fn reload(&mut self) -> Result<()> {
        // SAFETY: C ABI call, same instance as above.
        unsafe {
            let create: libloading::Symbol<FnCreateDeviceList> = self
                ._library
                .get(b"CreateDeviceList")
                .map_err(|err| BackendError::Io(err.to_string()))?;
            let _ = create();
        }
        let mut render = DeviceInfo::default();
        let rc = unsafe { (self.get_by_id)(0, 0, &mut render, 0) };
        if rc == 0 {
            self.render = render;
        }
        let mut capture = DeviceInfo::default();
        let rc = unsafe { (self.get_by_id)(1, 0, &mut capture, 0) };
        self.capture = if rc == 0 { Some(capture) } else { None };
        let _ = self.init;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn device_for(&self, name: &str) -> Result<DeviceInfo> {
        if is_capture_property(name) {
            self.capture
                .ok_or_else(|| BackendError::PropertyUnavailable(name.to_string()))
        } else {
            Ok(self.render)
        }
    }

    /// Read one vendor property as an integer (`action = 0` GET).
    pub fn get(&self, name: &str) -> Result<i32> {
        let mut device = self.device_for(name)?;
        let wide = to_wide(name);
        let mut buffer = vec![0u8; 0x400];
        let mut slot = buffer.as_mut_ptr() as *mut c_void;
        let rc = unsafe {
            (self.property_control)(&mut device, wide.as_ptr(), &mut slot, 0, 0)
        };
        if rc != 0 {
            return Err(BackendError::PropertyUnavailable(format!(
                "{name} (rc={rc:#x})"
            )));
        }
        let text = read_wide_string(slot as *const u16);
        text.trim()
            .parse::<i32>()
            .map_err(|err| BackendError::PropertyUnavailable(format!("{name}: {text:?} ({err})")))
    }

    /// Write one vendor property (`action = 1` SET, value passed through the slot).
    pub fn set(&self, name: &str, value: i32) -> Result<()> {
        let mut device = self.device_for(name)?;
        let wide = to_wide(name);
        let mut raw = value as u32;
        let mut slot = (&mut raw as *mut u32).cast::<c_void>();
        let rc = unsafe {
            (self.property_control)(&mut device, wide.as_ptr(), &mut slot, 0, 1)
        };
        if rc != 0 {
            return Err(BackendError::PropertyUnavailable(format!(
                "{name} set (rc={rc:#x})"
            )));
        }
        Ok(())
    }

    /// Play one of the vendor's channel-identification sounds.
    pub fn play_test_sound(&self, path: &Path, channel: u32) -> Result<()> {
        type FnPlay = unsafe extern "system" fn(*const u16, u32, DeviceInfo) -> i32;
        let play: FnPlay = unsafe {
            let symbol: libloading::Symbol<FnPlay> = self
                ._library
                .get(b"PlayTestSound")
                .map_err(|err| BackendError::Io(err.to_string()))?;
            *symbol
        };
        let wide = to_wide(&path.to_string_lossy());
        let rc = unsafe { play(wide.as_ptr(), channel, self.render) };
        if rc != 0 {
            return Err(BackendError::Io(format!("PlayTestSound rc={rc:#x}")));
        }
        Ok(())
    }

    /// Stop any test sound started through the vendor library.
    pub fn stop_test_sound(&self) -> Result<()> {
        type FnStop = unsafe extern "system" fn() -> i32;
        let stop: FnStop = unsafe {
            let symbol: libloading::Symbol<FnStop> = self
                ._library
                .get(b"StopTestSound")
                .map_err(|err| BackendError::Io(err.to_string()))?;
            *symbol
        };
        let rc = unsafe { stop() };
        if rc != 0 {
            return Err(BackendError::Io(format!("StopTestSound rc={rc:#x}")));
        }
        Ok(())
    }
}

impl Drop for ConfApi {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.uninit)();
        }
    }
}

/// Capture-side vendor property names (mirrors the recovered ConfLib routing).
fn is_capture_property(name: &str) -> bool {
    name.starts_with("AI_")
        || name.starts_with("NR_")
        || name.starts_with("AEC_")
        || name.starts_with("Mic")
        || name.starts_with("MIC")
        || name.contains("MAGICVOICE")
        || name.starts_with("ENC_")
        || name.starts_with("Enable_NOISEREDUCTION")
        || name.starts_with("Enable_MIC")
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_wide_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    // SAFETY: ConfLib writes a NUL-terminated wide string of at most 0x200 chars.
    unsafe {
        while *ptr.add(len) != 0 && len < 0x400 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}
