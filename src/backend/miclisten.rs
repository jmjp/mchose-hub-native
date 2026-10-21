//! Microphone monitoring via the vendor `MicListen.dll`.
//!
//! The official application toggles Windows microphone monitoring through this
//! sidecar DLL (`ToggleListenDefault`, `ToggleListenById`, `ToggleListenByName`).
//! The exports return 0 on success. Whether the resulting loopback is audible was
//! not verified acoustically during this reverse-engineering effort, so the UI
//! reports the toggle as best-effort.

use std::path::PathBuf;
use std::sync::OnceLock;

use libloading::Library;

type FnToggleDefault = unsafe extern "system" fn(i32) -> i32;
type FnToggleByName = unsafe extern "system" fn(i32, *const std::ffi::c_char, *const std::ffi::c_char) -> i32;

static LIB: OnceLock<Option<Library>> = OnceLock::new();

fn library() -> Option<&'static Library> {
    LIB.get_or_init(|| match find_dll() {
        Some(path) => match unsafe { Library::new(&path) } {
            Ok(lib) => {
                log::info!("MicListen loaded from {}", path.display());
                Some(lib)
            }
            Err(err) => {
                log::warn!("cannot load MicListen from {}: {err}", path.display());
                None
            }
        },
        None => {
            log::info!("MicListen.dll not found (official MCHOSE HUB not installed)");
            None
        }
    })
    .as_ref()
}

fn find_dll() -> Option<PathBuf> {
    for dir in crate::config::vendor_install_dirs() {
        for rel in ["libs/DLL/MicListen.dll", "libs/MicListen.dll"] {
            let path = dir.join(rel);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// True when the vendor monitoring DLL is available.
pub fn available() -> bool {
    library().is_some()
}

/// Enable or disable Windows microphone monitoring.
///
/// `device_name` is the capture endpoint friendly name when known; the vendor
/// helper falls back to the default capture device otherwise.
pub fn set_listen(enabled: bool, device_name: Option<&str>) -> Result<(), String> {
    let lib = library().ok_or_else(|| "MicListen.dll is not available".to_string())?;
    let value = i32::from(enabled);
    unsafe {
        if let Some(name) = device_name {
            let name_c = std::ffi::CString::new(name).map_err(|err| err.to_string())?;
            let by_name: libloading::Symbol<FnToggleByName> = lib
                .get(b"ToggleListenByName")
                .map_err(|err| err.to_string())?;
            let rc = by_name(value, name_c.as_ptr(), std::ptr::null());
            if rc == 0 {
                return Ok(());
            }
            log::debug!("ToggleListenByName({name}) rc={rc}; falling back to default");
        }
        let default: libloading::Symbol<FnToggleDefault> = lib
            .get(b"ToggleListenDefault")
            .map_err(|err| err.to_string())?;
        let rc = default(value);
        if rc == 0 {
            Ok(())
        } else {
            Err(format!("ToggleListenDefault rc={rc:#x}"))
        }
    }
}
