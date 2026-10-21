//! Cross-platform "start with the operating system" support.
//!
//! * Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//! * Linux: XDG autostart desktop entry in `~/.config/autostart`.
//! * macOS: LaunchAgent property list in `~/Library/LaunchAgents`.

use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;

const VALUE_NAME: &str = "MHubNative";
const LINUX_FILE: &str = "mchose-hub-native.desktop";
const MAC_LABEL: &str = "com.mchose.hubnative";

/// True when the current platform implements auto-start.
pub fn supported() -> bool {
    cfg!(windows) || cfg!(target_os = "linux") || cfg!(target_os = "macos")
}

fn executable() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|err| err.to_string())
}

/// Enable or disable launching the app at login.
#[allow(clippy::needless_return)]
pub fn set(enabled: bool) -> Result<(), String> {
    let exe = executable()?;
    #[cfg(windows)]
    {
        let result = if enabled {
            Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    VALUE_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &exe.display().to_string(),
                    "/f",
                ])
                .output()
        } else {
            Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    VALUE_NAME,
                    "/f",
                ])
                .output()
        };
        return match result {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(String::from_utf8_lossy(&output.stderr).to_string()),
            Err(err) => Err(err.to_string()),
        };
    }

    #[cfg(target_os = "linux")]
    {
        let path = linux_autostart_path()?;
        if !enabled {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|err| err.to_string())?;
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let exec = format!("\"{}\"", exe.display());
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=M HUB Native\nComment=Native companion for MCHOSE headsets\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
        );
        return std::fs::write(&path, entry).map_err(|err| err.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let path = mac_agent_path()?;
        if !enabled {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|err| err.to_string())?;
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{MAC_LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
            exe.display()
        );
        return std::fs::write(&path, plist).map_err(|err| err.to_string());
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = exe;
        let _ = enabled;
        Err("auto-start is not implemented on this platform".to_string())
    }
}

/// Check whether auto-start is currently registered (best effort).
#[allow(clippy::needless_return)]
pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        let output = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                VALUE_NAME,
            ])
            .output();
        return output.map(|output| output.status.success()).unwrap_or(false);
    }
    #[cfg(target_os = "linux")]
    {
        return linux_autostart_path()
            .map(|path| path.exists())
            .unwrap_or(false);
    }
    #[cfg(target_os = "macos")]
    {
        return mac_agent_path().map(|path| path.exists()).unwrap_or(false);
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
fn linux_autostart_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| "cannot resolve the XDG config directory".to_string())?;
    Ok(base.join("autostart").join(LINUX_FILE))
}

#[cfg(target_os = "macos")]
fn mac_agent_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{MAC_LABEL}.plist")))
}
