//! Persistent application configuration and filesystem locations.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directory name used under `%APPDATA%` (kept separate from the original app).
const APP_DIR: &str = "MCHOSE_HUB_Native";

/// UI theme selection (mirrors the original `system.json` theme values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// User-visible settings persisted to `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Schema version, for future migrations.
    pub version: u32,
    /// UI language override. `None` = follow the OS UI language.
    pub language: Option<String>,
    pub theme: ThemeMode,
    /// Start the app automatically with Windows (registry `Run` key).
    pub auto_start: bool,
    /// Start minimized to the tray.
    pub start_minimized: bool,
    /// Closing the window hides it to the tray instead of exiting.
    pub close_to_tray: bool,
    /// Last window size in points.
    pub window_size: [f32; 2],
    /// Last selected page id.
    pub last_page: Option<String>,
    /// Last selected EQ preset id.
    pub selected_preset: Option<String>,
    /// Log level (`error`, `warn`, `info`, `debug`, `trace`).
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            language: None,
            theme: ThemeMode::System,
            auto_start: false,
            start_minimized: false,
            close_to_tray: true,
            window_size: [1120.0, 720.0],
            last_page: None,
            selected_preset: None,
            log_level: "info".into(),
        }
    }
}

impl AppConfig {
    /// Load `config.json`, returning defaults when absent or unreadable.
    pub fn load() -> Self {
        let path = config_file();
        match fs::read_to_string(&path) {
            Ok(text) => {
                // Tolerate a UTF-8 BOM some editors/PowerShell versions write.
                let text = text.trim_start_matches('\u{feff}');
                match serde_json::from_str::<AppConfig>(text) {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        log::warn!("config.json is invalid ({err}); using defaults");
                        Self::default()
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                log::warn!("cannot read config.json: {err}; using defaults");
                Self::default()
            }
        }
    }

    /// Persist the configuration atomically.
    pub fn save(&self) -> io::Result<()> {
        let dir = data_dir();
        fs::create_dir_all(&dir)?;
        let path = config_file();
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(&tmp, text)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}

/// True when Windows is configured with the dark app theme.
///
/// Reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`
/// so the `System` theme mode really follows the machine (egui's default visuals are
/// dark, which previously made `System` always dark).
#[cfg(windows)]
pub fn system_prefers_dark() -> bool {
    use std::ffi::c_void;
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };
    unsafe {
        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        );
        status == ERROR_SUCCESS && value == 0
    }
}

#[cfg(target_os = "macos")]
pub fn system_prefers_dark() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("dark")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn system_prefers_dark() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("dark")
        })
        .unwrap_or(false)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn system_prefers_dark() -> bool {
    false
}

/// Root of the app's own data directory (`%APPDATA%\MCHOSE_HUB_Native`).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| dirs_home().map(|h| h.join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn preset_file() -> PathBuf {
    data_dir().join("eq_presets.json")
}

/// Candidate install directories of the original MCHOSE HUB (vendor components and
/// test-sound assets live there). Only used when the user has the original installed.
pub fn vendor_install_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(pf).join("MCHOSE HUB"));
    }
    if let Some(pf) = std::env::var_os("ProgramW6432") {
        let p = PathBuf::from(pf).join("MCHOSE HUB");
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs.push(PathBuf::from(r"C:\Program Files\MCHOSE HUB"));
    dirs
}

/// Locate the C-Media ConfLib DLL shipped with the original application, if present.
pub fn find_conf_lib() -> Option<PathBuf> {
    for dir in vendor_install_dirs() {
        for rel in ["libs/osConfLib.dll", "osConfLib.dll"] {
            let candidate = dir.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Locate the vendor's 7.1 test-sound WAV for a channel, if the original app is installed.
pub fn find_test_sound(wav_name: &str) -> Option<PathBuf> {
    for dir in vendor_install_dirs() {
        let candidate = dir.join("libs").join("sound").join(wav_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Resolve the OS UI language as a BCP-47-ish tag ("en", "pt", "zh", ...).
pub fn system_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() {
                let tag = value.split(['.', '@']).next().unwrap_or(&value);
                let short = tag.split(['_', '-']).next().unwrap_or(tag);
                if !short.is_empty() {
                    return short.to_lowercase();
                }
            }
        }
    }
    // Windows: use the locale name from the registry-backed culture info via env
    // fallback (`GetUserDefaultLocaleName` is not worth an extra dependency here).
    "en".to_string()
}

/// Ensure the data directory exists (called once at startup).
pub fn ensure_dirs() -> io::Result<()> {
    fs::create_dir_all(data_dir())?;
    fs::create_dir_all(log_dir())?;
    Ok(())
}

/// Human-readable path for the UI ("open data folder" button).
pub fn data_dir_display() -> String {
    data_dir().display().to_string()
}

/// Check whether a path looks like the original vendor install.
pub fn detect_vendor_install() -> Option<&'static Path> {
    None
}
