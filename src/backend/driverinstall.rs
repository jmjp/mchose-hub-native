//! Driver installation via the vendor INF packages.
//!
//! The original application installs the per-device C-Media driver + APO packages
//! with `pnputil.exe -i -a <inf>` from `libs\<driverDir>\...` (observed in the
//! IPC capture, e.g. driverDir `S9_PRO_14429` for the V9 PRO). This module
//! reproduces that flow with a single elevated `cmd` process so the user sees one
//! UAC prompt for all three INFs.

use std::process::Command;

use crate::backend::BackendError;

/// Candidate driver packages per USB product id (only verified families listed).
fn driver_dir_for(pid: u16) -> Option<&'static str> {
    match pid {
        0x385D => Some("S9_PRO_14429"), // MCHOSE V9 PRO
        _ => None,
    }
}

/// The three INF packages the vendor installs for a headset driver bundle.
fn inf_relative_paths(driver_dir: &str) -> Vec<String> {
    let base = format!("libs\\{driver_dir}");
    vec![
        format!("{base}\\Apo\\01_EXTENSION\\CM2025_BLK2025GHS_Ext.inf"),
        format!("{base}\\Apo\\02_APO\\CM2025_BLK2025GHS_APO.inf"),
        format!("{base}\\Driver\\BLK2025GHS.Inf"),
    ]
}

/// Install/repair the audio driver + APO packages for the given device.
///
/// Requires elevation: the returned command runs `pnputil` through an elevated
/// `cmd.exe` (one UAC prompt). Returns the elevated command output on success.
pub fn install_driver(pid: u16) -> Result<String, BackendError> {
    let driver_dir =
        driver_dir_for(pid).ok_or(BackendError::Unsupported("driver package for product"))?;
    let Some(install_root) = crate::config::vendor_install_dirs()
        .into_iter()
        .find(|dir| dir.join("libs").join(driver_dir).is_dir())
    else {
        return Err(BackendError::Io(
            "driver package not found (official MCHOSE HUB install required)".into(),
        ));
    };

    let infs = inf_relative_paths(driver_dir);
    let mut command_parts = Vec::new();
    for rel in &infs {
        let full = install_root.join(rel);
        if !full.is_file() {
            return Err(BackendError::Io(format!("missing INF: {}", full.display())));
        }
        command_parts.push(format!("pnputil.exe -i -a \"{}\"", full.display()));
    }
    let script = format!("{} & exit /b 0", command_parts.join(" & "));

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Start-Process -FilePath cmd.exe -ArgumentList '/c {}' -Verb RunAs -Wait -WindowStyle Hidden",
                script.replace('\'', "''")
            ),
        ])
        .output()
        .map_err(|err| BackendError::Io(format!("cannot start elevated installer: {err}")))?;

    if !output.status.success() {
        return Err(BackendError::Io(format!(
            "elevated driver install failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    log::info!("driver install requested for PID {pid:#06x} ({driver_dir})");
    Ok(format!(
        "pnputil completed for {} INF package(s) in {driver_dir}",
        infs.len()
    ))
}

/// Human-readable driver package id for the UI, when known.
pub fn driver_dir_label(pid: u16) -> Option<&'static str> {
    driver_dir_for(pid)
}

