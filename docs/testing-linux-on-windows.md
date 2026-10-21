# Testing the Linux build on Windows 11 (without touching your Windows install)

There are four practical ways to run the Linux binary on a Windows 11 machine.
All of them are removable; the "cleanup" column tells you exactly what to delete.

| Option | Best for | Needs | Cleanup |
| --- | --- | --- | --- |
| **WSL2 + Ubuntu 24.04** | fastest way to run the binary + GUI | Windows feature WSL2 | `wsl --unregister Ubuntu-24.04` (features can be disabled again) |
| **Bazzite in VirtualBox** | a real Bazzite desktop, USB passthrough possible | VirtualBox install | delete VM folder + uninstall VirtualBox |
| **Bazzite in Hyper-V** | no third-party hypervisor (Windows 11 Pro) | Hyper-V feature | delete VM + VHDX/ISO; disable Hyper-V |
| **Bazzite live USB** | testing on real hardware (GPU/audio/USB) | 16 GB USB stick | remove USB, delete UEFI boot entry |

Bazzite is an immutable Fedora-based distro: you can run the portable binary as-is
(`chmod +x`, run), and anything installed with `rpm-ostree install` can be reverted
with `rpm-ostree reset`.

---

## 1. WSL2 (fastest; not Bazzite)

Windows 11 with WSL2 also runs Linux GUI apps through WSLg.

```powershell
wsl --install -d Ubuntu-24.04
# reboot if prompted, then open "Ubuntu 24.04" from the Start menu
```

Inside Ubuntu:

```bash
sudo apt update
sudo apt install -y libxkbcommon0 libxkbcommon-x11-0 libgl1 libegl1 \
  libwayland-client0 libx11-6 libxcursor1 libxrandr2 libxi6 libudev1
curl -fL -o mhub \
  https://github.com/jmjp/mchose-hub-native/releases/latest/download/mchose-hub-native-linux-x86_64
chmod +x mhub
./mhub --probe          # headless protocol/audio diagnostic (JSON)
./mhub                  # opens the egui window through WSLg
```

Notes:
- No MCHOSE headset inside WSL by default (UI shows "My devices" empty). USB passthrough
  is possible with `usbipd-win`, but the Windows HID driver usually owns the device and
  the vendor DSP is Windows-only anyway.
- Audio inside WSLg is limited; the app falls back to volume/mute via `pactl` only if a
  PulseAudio server is reachable inside the distro.

Cleanup:

```powershell
wsl --shutdown
wsl --unregister Ubuntu-24.04
```

Optionally also remove the features (requires reboot):

```powershell
dism /online /disable-feature /featurename:Microsoft-Windows-Subsystem-Linux /norestart
dism /online /disable-feature /featurename:VirtualMachinePlatform /norestart
```

---

## 2. Bazzite in VirtualBox (recommended for a real Bazzite desktop)

VirtualBox supports **USB passthrough** (with the Extension Pack), so it is the only
VM option that can expose the headset to the guest — useful if you want to test device
discovery/HID, not only the UI.

1. Download a Bazzite desktop ISO (`bazzite.gg` → e.g. KDE or GNOME, `amd64`).
2. Install VirtualBox + Extension Pack.
3. Create a VM: **Fedora (64-bit)**, 8 GB RAM, 4 CPUs, 40 GB disk, EFI enabled,
   attach the ISO, enable 3D acceleration (VMSVGA).
4. Boot → choose the **live/try** session (do not install) → open a terminal:

```bash
curl -fL -o mhub \
  https://github.com/jmjp/mchose-hub-native/releases/latest/download/mchose-hub-native-linux-x86_64
chmod +x mhub
./mhub --probe
./mhub
```

5. Optional (headset passthrough): VirtualBox → VM Settings → USB → enable USB 3.0,
   then attach `291d:385d`. Inside Bazzite add a udev rule so your user can open the
   hidraw node:

```bash
sudo tee /etc/udev/rules.d/99-mchose.rules >/dev/null <<'EOF'
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="291d", ATTRS{idProduct}=="385d", MODE="0660", TAG+="uaccess"
EOF
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Bazzite uses PipeWire, so the Volume/Mute controls should work (`wpctl` is present).

Cleanup: delete the VM and its disk/ISO files, then uninstall VirtualBox. Nothing else
is left on Windows.

---

## 3. Bazzite in Hyper-V (Windows 11 Pro, no third-party software)

```powershell
dism /online /enable-feature /featurename:Microsoft-Hyper-V-All /all /norestart
# reboot
```

Hyper-V Manager → New → Virtual Machine → **Generation 2**, 8 GB RAM, 40 GB disk,
attach the Bazzite ISO, then in VM Settings → Security **disable Secure Boot** (or enroll
Bazzite's key), and in Firmware set the DVD first. Boot the live session, open a terminal,
download and run the binary as in the VirtualBox recipe.

Caveat: Hyper-V has **no simple USB passthrough**, so the headset cannot be attached to
the guest; this option is for UI/audio testing only.

Cleanup: delete the VM (and "Delete virtual hard disk"), delete the ISO, then optionally:

```powershell
dism /online /disable-feature /featurename:Microsoft-Hyper-V-All /norestart
```

---

## 4. Bazzite live USB (real hardware, no install)

This is the only way to test the actual GPU/audio/USB path on your machine, and it is
still fully removable:

1. Use **Ventoy** (install it to the USB stick only) and copy the Bazzite ISO to it.
2. Suspend BitLocker if enabled; reboot into the USB (boot menu).
3. Choose the **live/try** session. Bazzite's live session includes the desktop and
   PipeWire; run the binary as above.
4. Do **not** start the installer.
5. Shut down, remove the USB. Optionally delete the UEFI boot entry with
   `efibootmgr` (Linux) or from the firmware setup.

Risks to be aware of: Secure Boot may need to be disabled or a MOK enrolled, and
changing the boot order touches firmware settings. Avoid the installer unless you
actually want to install Bazzite.

---

## What works in each option

| | WSL2 | Bazzite VM | Bazzite live USB |
| --- | --- | --- | --- |
| GUI (egui) | ✅ WSLg | ✅ VM console | ✅ |
| Volume/mute (system mixer) | ⚠️ depends on PulseAudio in WSL | ✅ PipeWire | ✅ |
| HID device discovery | ❌ (USB passthrough possible but fragile) | ⚠️ VirtualBox only | ✅ real device |
| Vendor DSP (EQ/effects) | ❌ | ❌ | ❌ (Windows-only) |
| Clean removal | ✅ unregister distro | ✅ delete VM | ⚠️ remove USB + boot entry |
