# MCHOSE HUB — Reverse-Engineering Report

Status: **draft in progress** (Lead-maintained). Sub-documents:

| Document | Content |
| --- | --- |
| `findings-renderer.md` | Renderer (remote web app) UI structure, routes, IPC, schemas |
| `findings-runtime.md` | Live runtime behavior, windows/tray, IPC traces, HID/audio evidence |
| `findings-native.md` | Native components: osConfLib (C-Media ConfLib), G9OTA (dongle SDK), helper DLL/EXEs, THX |
| `findings-prior-art.md` | Recovered protocol knowledge from the user's earlier MCHOSE projects |
| `plan.md` | Implementation plan for the Rust + egui reimplementation |

All claims below are labelled **Observed** (directly seen in shipped artifacts / runtime),
**Inferred** (derived from evidence, not directly proven) or **Unknown**.

---

## 1. Target identity

| Item | Value | Evidence |
| --- | --- | --- |
| Product | MCHOSE HUB (a.k.a. "M HUB") | PE version info / window title |
| Vendor | 迈从科技有限公司 (MCHOSE / Maicong) | PE version info |
| Install path | `C:\Program Files\MCHOSE HUB\` | filesystem |
| Main executable | `MCHOSE HUB.exe` — 181,000,704 bytes, PE32+ x86-64 | PE header |
| SHA-256 (exe) | `45d380f2be5c05a2ba6575c4075c4807cbe02d9920c2ebd06a0b477bd5295710` | REA `inspect_electron_fuses` |
| Version | FileVersion 1.0.0.0; app package version `1.0.0` | PE info, `app.asar/package.json` |
| Runtime | Electron 31.7.4 · Chromium 126.0.6478.234 · Node 20.18.0 · V8 12.6 | strings in `MCHOSE HUB.exe` |
| Platform | Windows (x86-64); updater/app-update.yml present | artifact |

The application is an Electron shell around **remote web content** plus a set of
**vendor native libraries**. It is not a normal self-contained Electron app: the
main-process code ships as V8 bytecode and the renderer UI is downloaded from MCHOSE
CDN servers at runtime.

---

## 2. Artifact inventory

### 2.1 Electron shell (`C:\Program Files\MCHOSE HUB\`)

| Path | Size | Role |
| --- | --- | --- |
| `MCHOSE HUB.exe` | 181.0 MB | Electron main binary (fuses below) |
| `MCHOSE HUB Launcher.exe` | 524 KB | launcher (starts/repairs the app) — Observed name, role inferred from name/logs |
| `MCHOSE HUB Watcher.exe` | 705 KB | watchdog + HID plug watcher; writes `%APPDATA%\MCHOSEHUB\files\logs\watcher.log` |
| `update.ps1`, `move_file.ps1`, `Uninstall MCHOSE HUB.exe` | — | updater/uninstaller scripts |
| `resources\app.asar` | 93,488,963 B | application bundle (see 2.2) |
| `resources\app-update.yml` | 95 B | generic updater, placeholder URL `https://example.com/auto-updates` |
| `resources\elevate.exe` | 107,520 B | UAC elevation helper |
| `libs\` | ~350 MB total | vendor native libraries, drivers, APO packages, THX media, sounds |
| `locales\`, `*.pak`, `*.dll`, `*.bin` | — | standard Electron/Chromium payload |

Electron fuses (Observed, REA `inspect_electron_fuses`):

```
RunAsNode                              enabled
EnableNodeOptionsEnvironmentVariable    enabled
EnableNodeCliInspectArguments          enabled
EnableEmbeddedAsarIntegrityValidation  disabled
OnlyLoadAppFromAsar                    disabled
LoadBrowserProcessSpecificV8Snapshot   disabled
GrantFileProtocolExtraPrivileges       enabled
```

Consequence: `--inspect` / `--remote-debugging-port` are honoured (used for runtime capture),
and asar contents are not integrity-checked.

### 2.2 `app.asar` (extracted at `mchose-hub-re/asar/`)

| Path | Size | Role |
| --- | --- | --- |
| `package.json` | — | `name: MCHOSEHUB`, `main: ./out/main/index.js`; deps: `@grpc/grpc-js`, `axios`, `dayjs`, `fs-extra`, `i18next`, `iconv-lite`, `jszip`, `koffi` (FFI), `usb` (libusb), `win-audio` |
| `out/main/index.jsc` | 929 KB | **main process**, V8 bytecode (bytenode-style cached data) |
| `out/main/bytecode-loader.cjs` | 2.3 KB | loader for `.jsc`; patches V8 flag hash + dummy source hash |
| `out/main/*.jsc` | 1–71 KB each | worker threads: `workerGetDevice` (dongle SDK), `workerUpgrade` ×2 (OTA), `sdk-worker` (C-Media audio SDK), `smtcLyricsWorker`, `workerAsarExtract`, `fake` SDK |
| `out/preload/index.js` | 26.8 KB | **preload bridge** exposing `window.electronAPI` (~230 IPC methods) — plain readable JS |
| `out/renderer/tips.html` + `tips-assets/*` | 739 KB JS | secondary "tips" BrowserWindow (Vue 3 + Element Plus) |
| `bundled/renderer/refresh.html` + `refresh-assets/*` | 1.4 MB | offline "refresh/updating" page used when the remote renderer cannot load |
| `lang/main.js`, `Lang/locales/index.js` | 3.4 KB | main-process i18n (tray labels, 9 languages) |
| `node_modules/` | 84.7 MB | `koffi` (win32 x64 FFI addon), `usb` (node-usb with prebuilt N-API addons), grpc, … |
| `resources/legacy-storage-probe.html` | — | legacy localStorage migration helper |

`out/main/index.js` is a 72-byte stub that `require`s `bytecode-loader.cjs` then
`index.jsc`. **No JavaScript source for the main process is present** and the V8
bytecode version (12.6) is newer than any public View8 disassembler build
(View8 supports V8 ≤ 11.3; its `VersionDetector` rejects the source hash `a5e2d63c`).
So the main process could only be studied through string extraction, preload API,
runtime inspection and native analysis — **decompiled source is not available**
(this is a stated limitation, see §8).

### 2.3 Native/vendor payload (`libs\`)

Observed inventory (high level; details in `findings-native.md`):

| Component | Size | Role (Observed/Inferred) |
| --- | --- | --- |
| `osConfLib.dll` | 462 KB | C-Media **ConfLib_Win11** audio configuration library; 17 exports; string-keyed `PropertyControl` drives the C-Media driver via **KS properties** (volume, EQ, 7.1, mic DSP, effects, test sounds). **No HID imports.** |
| `DLL\G9OTA.dll` | 2.4 MB | vendor wireless-dongle SDK (`libdongle`): `read_online_device`, `iterAllHidDevice`, `ota_update_client`, `ota_update_dongle`; **the only native HID protocol code** (imports HID.DLL/SetupAPI). |
| `DLL\K20ProOTA.dll` | 1.0 MB | JieLi `jl_usb_scsi_windows` firmware-upgrade library (USB SCSI OTA) for K20 Pro. |
| `DLL\WindowMonitor.dll` | 741 KB | foreground game/app detection (`StartFocusWatch`, `IA_*` scan APIs) + `WindowMonitorConfig.xml` (7.4 MB app/game database). |
| `DLL\smtc_monitor.dll` | 245 KB | Windows SMTC media-session monitor (`smtc_start/stop/request_update`, JSON callback) for lyrics features. |
| `DLL\wyy.dll` | 1.3 MB | `BetterNCMII.dll` — NetEase Cloud Music plugin/patcher support. |
| `DLL\InfLink-rs.plugin` | 1.7 MB | Rust NetEase SMTC patcher plugin. |
| `DLL\MicListen.dll` | 47 KB | microphone monitoring (`toggle_listen`). |
| `DLL\thx-audio-player.dll` | 320 KB | THX spatial audio player (`THXAP_*`, WASAPI/AVRT). |
| `THX\proto\*.proto` | 20 KB | gRPC contracts for the THX audio service. |
| `EXE\MCHOSE HUB Agent.exe` | 563 KB | helper (no exports; k32/u32/shell32) — likely updater/driver-install agent. |
| `EXE\MCHOSE HUB Notifier.exe` | 686 KB | OSD/toast helper: Caps/Num/Scroll/Ins lock indicators, Win/Mac mode toast, low-battery toast (locales `.ini`). |
| `CM002X1, G9_PRO, G9_WIRED, K20, K20_GT, K20_PRO, RzlfVjI=, S9_PRO, S9_PRO_14429, V9, V9_PRO, VT008, VT009, X9` | per-device | Windows audio driver packages (`.inf/.sys/.cat`), APO effect DLLs (`CmApoIce.dll`), extension drivers — installed on demand. |
| `sound\*.wav`, `THX\media\7.1.wav/.mp4` | 110 MB | test sounds for channel/7.1 calibration. |
| `EXE\png\*` | — | OSD icons (`caps_on/off`, `num_*`, `scroll_*`, `ins_*`, `win`, `mac`). |

Supported device families visible in `libs\` (heads et al.): **V9, V9_PRO, V9_TURBO (VT008/VT009),
G9_PRO, G9_WIRED, G9_II (RzlfVjI=), S9_PRO, S9_PRO_14429, K20, K20_GT, K20_PRO, X9, CM002X1**.

---

## 3. Runtime architecture

### 3.1 Processes (Observed while the app was running)

```
MCHOSE HUB.exe (main, Electron)  PID 12052
├── GPU process
├── 2–3 renderer processes (custom scheme mchose-renderer://)
MCHOSE HUB Watcher.exe ×3   watchdog ("Guardian"/"Worker") + HID plug watcher
MCHOSE HUB Agent.exe        helper
MCHOSE HUB Notifier.exe     OSD/toast window + chimes
```

`Watcher` writes `watcher.log` with a **HID plug/ack protocol** for keyboard-family
devices (vendor `VID_3837`, PIDs `3020/302D/3030/303E`, `mi_02&col04`,
`read=0x0001/0x0000 ack=0x0001/0x0080`). This is separate from the connected headset
(V9 PRO = `VID_291D/PID_385D`).

### 3.2 Renderer loading (Observed in `%APPDATA%\MCHOSEHUB\files\log\*.txt`)

* The main window loads a **custom-scheme URL**:
  `mchose-renderer://app/cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26`
  which the main process proxies to `https://cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26`.
* A refresh page (`app.asar/bundled/renderer/refresh.html`) is shown while the remote
  page is probed/downloaded; if the remote load fails, the refresh page stays.
* Renderer version is negotiated from the renderer itself (`renderer version synced
  from renderer process: 1.1.26`), stored in the app store (`rendererVersion`), and
  the load URL is retried with the version query.
* A **tray BrowserWindow** loads `https://www.mchose.com.cn/tray.html`; two modes
  exist: `contextMenu` (native tray menu) and `customWindow` (HTML tray window).
* Dev/alternate origins exist in the main bundle strings (`192.168.x.x`, `localhost:5173`,
  `drivertest.maicong.cn`, `www.mchose.com.cn`, `61.141.76.237:9966`), selected by an
  environment/config mechanism (`getTargetEnvConfig`, `getEnvConfigs`, `getCurEnv`).

### 3.3 Audio/device control data flow (Observed + Inferred)

```
renderer (remote Vue app)
  │  ① audio settings
  ├─ window.electronAPI.* ──IPC──► main process ──► koffi ──► osConfLib.dll
  │                                                   └─► C-Media audio driver (KS properties)
  │  ② device info / OTA / dongle
  ├─ IPC ──► worker_threads ──► koffi ──► G9OTA.dll (libdongle, HID) / K20ProOTA.dll (JieLi SCSI)
  │  ③ device settings (per-device layer)
  └─ WebHID in renderer (navigator.hid; Electron select-hid-device) ──► HID collections
       0xFF82 report 65 command channel, 0xFF90 report 85 notify, 0xFF22 feature 170 firmware
```

`out/preload/index.js` has **no generic HID transfer IPC** — only descriptor/version/
firmware-image helpers — which is consistent with the renderer using WebHID directly
(electron grants HID device access to the trusted renderer). Runtime capture must
confirm this (see `findings-runtime.md`).

### 3.4 Startup sequence (Observed in logs)

1. Chromium user-data dir `%APPDATA%\MCHOSEHUB` (profile, Cache, IndexedDB, Local Storage).
2. Main store cache load (`files\config\*.json`) → `MainStore 初始化完成`.
3. Default config initialisation; store watchers; tray manager init; tray window creation.
4. Main window: probe remote renderer → load via `mchose-renderer://` → switch tray to
   `customWindow` mode; `sysLang` detected (e.g. `pt-BR`).
5. Autostart behaviour: `最小化到托盘自启动` (start minimized to tray when launched by autostart).
6. Shutdown: `APP Lifecycle: before-quit dispose THX complete`, audio SDK manager dispose.

---

## 4. Configuration & persistence (Observed)

App store/cache dir: `%APPDATA%\MCHOSEHUB\files\config\`

| File | Content |
| --- | --- |
| `system.json` | app settings: `firstRun, autoStart, autoUpdate, autoStartMini, theme ("light"), hideWhenClose, bgImgMode, bgImgModeL2, bgImgPath, bgImgBlur, bgCardBlur, uiScaleLevel, lang ("en_US"), bgWallpaperV2{wallpaperId}, bgImgBrightness` |
| `mc_main_store_key.json` (31.7 KB) | main-process store dump: `user_config{token,nickname,headimgurl}`, `net-config-global{customFields{headerBtnConfigs, expertViewConfig, bgConfigV2 (wallpapers incl. limited-time events, official/team/fanren categories), gtConfig (showGc, supportedAppNames: qqmusic/cloudmusic), isRemoteLang, firstGuide (per-device onboarding), version table (per device config hashes: keyboardPreset, mouseConfig, otaConfigApp_prod, …)}, pcVersion{tag 1.2.05}, webVersion{tag v3.16.32}, rendererVersion{tag 1.1.26}, refreshTime, webIndexList[device descriptors]}`, `legacy_*_migrated_origins`, `webIndexListLen` |
| `webIndexList[0]` (device descriptor) | the connected V9 PRO: `vendorId 10525, productId 14429, productKey "MCHOSE V9 PRO$$$10525$$$14429", deviceName "V9 Pro", deviceType 3, connectMode 1 (wireless), showBattery true, enableBatteryVersion "0008", isElectronDevice true, batteryPrecision 10, firstGuideType "disabled", firmware{headsetVersion "0024", dongleVersion "0004"}`, color variants with CDN image URLs |
| `eq.json` | shipped **10-band EQ preset database**: categories `game` (Default, 无畏契约/CS/Apex/PUBG/CF… ×3 each, 10 ints each) and `music` (舞曲/爵士/重金属/摇滚/…), plus `current` map keyed by `MCHOSE V9 PRO PID14429`. Preset values are integers roughly **−14 … +14** (0.1 dB-ish UI units). |
| `shared_cache.dat`, `files\logs\watcher.log`, `files\log\YYYYMMDD.txt` | shared cache blob, watcher log, daily main log |

Chromium profile data (Local Storage, IndexedDB `mchose-renderer_app`, Cache) is under
`%APPDATA%\MCHOSEHUB\` and contains renderer state and cached remote assets.
Do NOT assume `files\config\eq.json` is the only EQ user data — the renderer also keeps
per-device EQ in its own storage (see `findings-renderer.md`).

---

## 5. Feature surface (Observed: `out/preload/index.js` IPC contract)

The preload bridge is the authoritative list of main-process capabilities. Grouped:

### 5.1 Window / app / system
`getVersion, getEnv, getArgv, getMainFlags/setMainFlags, getIsAdminRunning, isInstalledInSystemDrive,
restartComputer, getAppMetrics, openBrowser, openDevTools, quit`
`closeWindow/hideWindow/minimizeWindow/maximizeWindow/unmaximize/toggleMax/isMaximized,
isWindowVisible(id), getWindowStateSnapshot(id), getWindowSize/setWindowSize/setWindowMInSize,
setZoomFactor, setBounds, setPosition, getDragBoundsAndRestoreIfMaximized,
getScreenInfo, getWindowsTheme/getThemeValue`
`windowManager(params), customTray(params), createTrayWindow(params, useOldTray)`
`setAutoStart/getAutoStartStatus, getConfig/setConfig, getAllStore/setStore/getStore/hasStore/
deleteStore/clearStore/watchAllStore/unwatchAllStore, getSharedConfig/setSharedConfig,
getUserConfig/setUserConfig, getLegacy* (localStorage/IndexedDB/custom images migration),
localFiles{getRoot,read,write,delete,exists,mkdir,list}, readLocalFile, openUserFilesDir,
showOpenFileDialog, importData/exportData, saveBgImg/getBgImg/deleteFile/saveGifImg/getGifImgList,
getDefaultLang/setMainLang/getLang, getRendererUrl, getRefreshPageContext, refreshGlobalConfig,
probeRendererReachable, retryLoadRenderer, checkVcRedistX64`

### 5.2 Audio DSP / effects (maps 1:1 onto osConfLib `PropertyControl` names)
`setToDefault, getVolumeScalarControl/setVolumeScalarControl(channel,val,type),
getMuteControl/setMuteControl(muted,type), openSoundSetting`
`getAINREnable/setAINREnable, getNR_Enable/setNR_Enable,
getEnable_MAGICVOICE/setEnable_MAGICVOICE, getMagicVoice_Selection/setMagicVoice_Selection`
`getEnable_AUDIOBRILLIANT_LFX / set…, getAUDIOBRILLIANT_LEVEL / set…`
`getEnable_Channel_COPY_LFX / set…`
`getEnable_VIRTUALBASS_LFX / set…, getVIRTUALBASS_Level / set…,
getVIRTUALBASS_CutOffFrequency / set…`
`getEnable_ADAPTIVEVOLUME_GFX / set…, getADAPTIVEVOLUME_LEVEL / set…, getADAPTIVEVOLUME_MODE / set…`
`getEnable_VOICECLARITY_LFX / set…, getVOICECLARITY_LEVEL / set…,
getVOICECLARITY_NOISESUPP_LEVEL / set…`
`getXearSurroundEnable/getXearSurroundRoom/getXearSurroundMode + setters`
`GetEnable_EQ_GFX/setEnable_EQ_GFX, getEQ_Slider(index)/setEQ_Slider(index,value)`
`playTestSound/stopTestSound, openScreenSetting, openWindowsMouseSetting,
startWatchAudioVolume/stopWatchAudioVolume, getAudioVolume/setAudioVolume(p),
getAudioMute/setAudioMute(p), getMicListenStatus/setMicListenStatus`

### 5.3 Audio SDK / THX spatial
`initDll(name), AudioSDKGetState, AudioSDKInstallDriver, AudioSDKReleaseDevice,
AudioSDKEnsureReady(registration), AudioSDKCall(method, args)`
`AudioTHXGetState, AudioTHXGetInstallState, AudioTHXGet7TestVideoInfo,
AudioTHXListAudioPlayerProducts/Endpoints, AudioTHXResolveAudioPlayerEndpointGuid,
AudioTHXAudioPlayerControl, AudioTHXDownloadInstaller/GetDownloadInstallerState/InstallInstaller,
AudioTHXUninstallBundle, AudioTHXGetRecoveryState, AudioTHXCheckAfunix,
AudioTHXRunServiceRepair, AudioTHXEnsureReady, AudioTHXCall,
AudioTHXReleaseDevice, AudioTHXReleaseClientShellByHwid,
AudioTHXClearAudioPlayerEndpointByHwid, AudioTHXSubscribePropertyChanges/Unsubscribe`

### 5.4 Device / firmware / OTA
`getUsbDeviceDescriptor(p), getUsbStringDescriptor(p), getUsbVersion(p),
getFWVersionFromUsbDescriptor(p), getFirmwareVer, getFirmwareBinData(p),
getDeviceFriendlyName(type), otaDevice(p), installDriver(p), uninstallDriver,
getUpdaterState / updater API (startSilentDownload, startManualUpdate, applyDownloadedUpdateNow, onStateChanged)`

### 5.5 App/game integration, media, extras
`scanAppList, scanGameList, getExeDetailInfo, startListenActiveAppChange/stop…,
startLyricsMonitor/stopLyricsMonitor/isLyricsMonitorRunning,
readQQMusicQrc/resolveQQMusicLyricPaths, readKuGouKrc/resolveKuGouLyricPath,
getKuGouPlaybackState, getNcmInstallPath, installBetterNcm/uninstallBetterNcm,
removeInfLinkPlugin, recorderCall(methodName, param), showWinToast(p), onBatteryInfo(cb)`

### 5.6 Push events (main → renderer)
`battery_info, callbackVOLUMEANDMUTE, onDllReady, onRecorderData, onRecorderError,
onAudioVolumeChange, onActiveAppChange, onLyricsMediaUpdate, betterNcmInstallProgress,
storeChange, onOtaMessage, onIframeLoadFailed, windowStateChange, THX property changes
(+ended/error), maximize/unmaximize, downloadProgress`

Feature interpretation (Inferred, from names + configs + native DLL exports):

* **Sound page**: 10-band EQ with presets (game/music), enable/disable, plus three
  enhancement blocks (brilliant / channel copy / virtual bass) and "smart volume".
* **7.1 / spatial**: Xear surround (enable, room size, mode) and THX spatial audio
  (install/repair/state, endpoint binding, 7.1 test video).
* **Mic page**: AI noise reduction, NR, magic voice (selection), voice clarity + noise
  suppression, mic listen/monitoring.
* **Home/device**: connection state, product image/colour variant (CDN), battery,
  firmware versions (headset + dongle), first-guide slides, device switching.
* **Settings**: theme (light/dark/auto), wallpaper (official/limited/team/custom image/video),
  background blur/brightness, UI scale, language (9 languages), autostart/start-minimised,
  close-to-tray, update check, driver install/repair, logs, about.
* **Extras**: keyboard lock-key OSD (Notifier), low-battery toast, per-app/game profiles
  (WindowMonitor), music lyrics display via SMTC (QQ Music / KuGou / NetEase Cloud Music +
  BetterNCM plugin), test sounds/channel identification, firmware update.

---

## 6. Device support matrix (Observed from `libs\` + store)

* Connected test device: **MCHOSE V9 PRO** headset — USB `VID 0x291D / PID 0x385D`,
  serial `0123456789AB`, composite: MI_00 HID (6 collections: consumer/telephony/vendor
  0xFF22/0xFF90/0xFF82), MI_01 C-Media USB audio. Audio endpoints
  `Alto-falantes (MCHOSE V9 PRO)` / `Mic (MCHOSE V9 PRO)`; driver package `BLK2025GHS`
  (`libs\S9_PRO_14429` shares the APO `CmApoIce.dll` for PID 14429).
* Supported families evidenced by driver/APO packages and store version keys: V9, V9 PRO,
  V9 Turbo/V9 Turbo+ (`VT008/VT009`), V9 PRO 2, G9 PRO, G9 V2, G9 wired, S9 PRO, S9 PRO
  (PID 14429), K20, K20 GT, K20 PRO, X9, R9, and keyboard/mouse families (Ace 75, God 60,
  A7 V3 Ultra+, R9 …) via remote per-device pages (`cdn.mchose.com.cn/customPage/<id>/index.html`).

The connected V9 PRO's HID protocol (prior art + runtime capture) is documented in
`findings-prior-art.md`: command report id 65 (`0xAA` frame, XOR checksum, cmdType pairs),
notify report id 85, firmware feature report id 170. Note the prior project concluded that
this SKU's *settings* are mostly host-side C-Media DSP (APO/KS properties) rather than
HID commands, while HID remains the identification/notification channel — the runtime
capture in this project is meant to confirm or refute that.

---

## 7. What we can and cannot reproduce natively

| Original capability | Reproducible in Rust? | Notes |
| --- | --- | --- |
| Device detection / identity / firmware version | **Yes** | `hidapi` enumeration by VID/PID, feature report 170; `SetupAPI` descriptors |
| Battery / connection status | Likely yes | via HID notify channel or dongle SDK; needs runtime confirmation |
| 10-band EQ, effects toggles, 7.1 Xear, mic DSP | **Yes, but through Windows/KS audio APIs** | `PropertyControl` maps names → KS property sets on the C-Media driver. Two implementation options: (a) pure Rust using `IKsControl`/`KSPROPERTY` with GUIDs/IDs recovered from `osConfLib.dll`; (b) FFI adapter to vendor `osConfLib.dll` (needs the original install). Decision documented in `plan.md`. |
| THX spatial audio service | **No (not self-contained)** | proprietary gRPC service/installer; UI can show state and link to setup but not reimplement the renderer |
| Firmware OTA | **Not reimplemented** | vendor SDK (`G9OTA.dll`) + per-device firmware images; too risky to reverse fully. UI can report versions and point to the official app |
| Driver/APO installation | **No** | Windows driver packages + APO registration; out of scope |
| Per-app/game profiles, lyrics windows, NCM patching | Partially | foreground-window detection isolatable (GetForegroundWindow); music/lyrics integrations are app-specific and out of scope |
| OSD (lock keys/battery toasts) | **Yes** | native egui overlay window + `RegisterHotKey`/hooks; Notifier behaviour observed |
| Tray + background operation | **Yes** | `tray-icon` crate |
| Cloud config / wallpapers / account | **No (server-dependent)** | documented as removed; local theme support kept |

---

## 8. Limitations of this investigation

1. **No main-process source**: V8 12.6 bytecode; no public disassembler supports it
   (View8 ≤ V8 11.3; `VersionDetector` rejects hash `a5e2d63c`). Main-process logic was
   therefore studied via string extraction, the preload contract, stored configs, logs and
   runtime inspection. Function-level pseudocode of main is unavailable.
2. **REA MCP had no native provider registered on this host** (Ghidra unregistered,
   IDA/Kuna absent, Hopper unsupported on win32). Native analysis was done with a locally
   installed Ghidra 12.1.4 headless; REA native tools could not be used. (REA *did* provide
   Electron fuses, JS/process/archive inspection and evidence bookkeeping.)
3. **Renderer is remote and versioned**: UI structure is recovered from the CDN bundle
   `cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` (cached + live). It can change
   server-side independently of the installed app.
4. **Per-device HID command layer** may live in the remote renderer JS and/or vendor DLLs;
   the runtime capture is the primary evidence for the V9 PRO's actual command usage.
5. **No destructive tests** were performed (no OTA flashing, no driver install/uninstall,
   no factory reset).
6. Vendor-specific protocols (THX gRPC service, JieLi SCSI OTA, APO property mapping) are
   only partially characterised.

---

## 9. Evidence index

| Evidence | Where |
| --- | --- |
| Installed app inventory, exe hash, fuses | this report §1–2; REA evidence `ev_78d7a6b1…` |
| Extracted asar | `mchose-hub-re/asar/` |
| Preload IPC contract | `mchose-hub-re/asar/out/preload/index.js` |
| Main bundle strings | `mchose-hub-re/strings/index.txt`, `index_ordered.txt` |
| Store/config dumps | `mchose-hub-re/captures/config-backup/` |
| App logs | `%APPDATA%\MCHOSEHUB\files\log\*.txt`, `files\logs\watcher.log` |
| Renderer assets + screenshots | `mchose-hub-re/renderer/`, `findings-renderer.md` |
| Runtime traces | `mchose-hub-re/captures/`, `findings-runtime.md` |
| Native evidence | `mchose-hub-re/captures/native/`, `findings-native.md` |
| Prior-art protocol work | `mchose-hub-re/prior-art/`, `findings-prior-art.md` |

*(Sections for renderer UI, runtime traces and native protocol details are merged from the
sub-documents once their captures complete.)*

---

## 10. Renderer / UI layer (summary of `findings-renderer.md`)

**Observed:** the installed client's UI is a remote Vue 3 + Pinia SPA served through
the custom scheme `mchose-renderer://app/...` from
`https://cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` (title `M HUB`),
byte-identical to the Chromium cache entry (`main-xA7mzCYt.js`, SHA-256 match).
The window is a thin shell; the `/index` "home" is itself an iframe that loads the
browser web-driver connect page (`https://www.mchose.com.cn/#/connectDevice`).

* **Native-surface routes** (the only screens the shell renders itself):
  `/#/index` (iframe host), `/#/theme` and 14 `/headset/*` screens — audio routine,
  equalizer, speaker effects, mic, surround (legacy Xear), THX spatial, other settings,
  lighting, screen (K20 GT), etc. (+ window chrome/tray/updater).
* **Two audio layout generations** per device: legacy `cmedia` (10-band EQ, Xear 7.1,
  LFX/GFX properties) and new `thx` (THX runtime + on-device HidEQ). The connected
  V9 PRO (10525/14429) is `cmedia` + legacy.
* **Device I/O in the renderer is WebHID** (`navigator.hid.sendReport`/`inputreport`),
  not an IPC bridge: 63-byte payload, `0xAA` start, version 1, cmdType pairs
  `[0,1]` request/response, `[0,2]` set, `[0,4]` notify, `[0,3]` image, `[0,6]` OTA;
  XOR checksum only when the flag bit is set (vendor sends 0). Report ids per family:
  **85 legacy headset**, 188 K20 family, 170 G9 V2. Notify command map (battery/sleep,
  EQ mode, light state, screen/lyrics, etc.) is documented in the sub-document.
* **Host-side audio** goes through the main process: `electronAPI.AudioSDKCall(method,
  [deviceId, ...args])` for `cmedia` and `AudioTHXCall({productKey, method, args})` for
  THX; a 56-case adapter table maps the legacy method names onto THX properties. The
  adapter is extracted verbatim in `renderer/notes/audio-method-map.json`.
* Per-feature specifications (ranges, defaults, labels, APIs) for the routine page,
  EQ (legacy + HidEQ PEQ), the four speaker effect cards, Xear surround, THX spatial,
  mic, other settings, lighting, screen and theme are in §7 of the sub-document;
  the full 9-locale i18n inventory and remote config schemas are included there too.

**Negative results:** the renderer cannot be screenshotted outside the Electron shell
(no `window.electronAPI` → blank page), and the browser web driver refuses audio
devices by design ("audio devices → download M HUB client"). Visual evidence must come
from the live client; `runtime-rec` captured it (see §12).

## 11. Native layer (summary of `findings-native.md`)

* `osConfLib.dll` = **C-Media ConfLib 1.0.8.0**, the whole audio-settings API behind one
  string-keyed dispatcher `PropertyControl(DEVICEINFO, wchar_t* name, void** out,
  void** in, uchar action)`. It resolves names to `PROPERTYKEY` and reads/writes an
  `IPropertyStore` as `VT_UI4`; 142 `name → {GUID, pid}` pairs across 23 vendor property
  sets were recovered (`captures/native/osconflib-property-table.json`). This is
  reimplemented natively in Rust (`backend/property_keys.rs` + `backend/winaudio.rs`).
* **Correction:** osConfLib has no HID imports; it is not the device-command layer.
* `G9OTA.dll` (`libdongle.dll`) is the only native HID code: enumeration via HID.DLL/
  SetupAPI and a 65-byte HID report transport (report id 0, 64-byte payload chunks,
  overlapped I/O) used by the JS workers for `read_online_device`/OTA. Its command
  layer is JieLi RCSP; opcodes/CRC were not fully recovered.
* `MCHOSE HUB Watcher.exe` is not a watchdog in the HID sense: it is the hardware
  "open the Hub" hotkey receiver (VID 0x3837 family, trigger report `0xC1`, ACK
  `0xC2`, configurable INI schema, launches/restores the main window).
* `Notifier.exe` = portable OSD server (lock keys, Win/Mac mode, low-battery toast)
  over `\\.\pipe\MCHOSEHUBOSD_Pipe` (`'LOSD'` message magic); `Agent.exe` is its pipe
  client. Battery source for the toast remains unknown.
* `K20ProOTA.dll` = JieLi USB-SCSI firmware upgrade library; `WindowMonitor.dll` =
  foreground game/app detection (`IA_*`); `smtc_monitor.dll` = SMTC media monitor;
  `MicListen.dll` = mic monitoring toggle; THX stack = `thx-audio-player.dll` +
  gRPC service (`.proto` contracts recovered); `wyy.dll`/`InfLink-rs.plugin` are
  NetEase Cloud Music integrations.
* Driver/APO packages for 16 device families ship under `libs\` but are installed
  on demand. **On the analysis machine the endpoint exposes no FX property store**,
  so C-Media DSP properties (EQ/effects) are unavailable — see §12.

## 12. Runtime evidence and hardware probes

* The original app was relaunched with `--remote-debugging-port=9222 --inspect=9229`
  in a separate profile (the long-running elevated instance cannot be killed by a
  filtered-admin token; scheduled tasks `MCHOSE_HUB_Launcher/Watcher` run at Highest).
  Renderer targets and the iframe target were found; renderer scripts were exported
  to `mchose-hub-re/renderer/live/`. Screenshots of the connect screen and device
  pages are in `mchose-hub-re/shots/`.
* **Native HID probe (`mchose-hub-native --probe`, hardware-observed):**
  * Topology: one HID interface, six collections — `0xFF82` (in+out report 65),
    `0xFF90` (in report 85; Windows caps exceed this and accept output 85),
    `0xFF22` (feature report 170), plus consumer/telephony collections.
  * Writes are accepted only on the matching (collection, report id): report 65 on
    `0xFF82`, report 85 on `0xFF90`; any mismatch fails with Win32 error 1
    ("incorrect function"). This explains the earlier WebHID conclusion that report
    85 had no output collection.
  * Firmware: feature report 170 replies to a `[0x01, target]` request with
    `AA 01 00 00 <4 version bytes>`; on the tested unit the bytes are not ASCII
    (headset `00 00 02 04`, dongle `00 00 00 04`) — the official app's published
    strings (`0024`/`0004`) could not be reconstructed from this reply.
  * Battery: neither the unframed `0x65` request nor framed `GET_BATTERY_SLEEP`
    (cmd 11) on either accepted report id produced any input report within 2.5 s,
    from either collection. Battery remains **unresolved**.
  * Audio: the default MCHOSE render endpoint exposes Core Audio volume/mute but no
    C-Media FX property store (`HKLM\...\MMDevices\Audio\Render\*\FxProperties` is
    empty for every endpoint), so EQ/effects/mic DSP property reads fail and the
    native app falls back to Core Audio. This is a driver/APO installation state, not
    a protocol error; the original app is subject to the same guard.

## 13. Prior art (summary of `findings-prior-art.md`)

Two earlier user projects were recovered from Claude Code session transcripts:
the `mchose-web` JS WebHID driver with a full protocol reference, and
`mchose-v9pro-rs`, a Rust/egui companion. They provide an independent, partly
hardware-verified account of the frame format, CmdType pairs, EQ/battery/firmware
commands, HID report map and the "host-side DSP" architecture conclusion. Differences
and contradictions (report id 65 vs 85; whether the firmware responds to framed
commands; the vendor EQ product-list gating) are catalogued in the sub-document.
The reimplementation uses the prior art as a cross-check only; every protocol value
it relies on is also present in the shipped renderer bundle (`findings-renderer.md` §10).

## 14. Reimplementation outcome (`mchose-hub-rs/`)

Delivered: a native Rust/egui app (`mchose-hub-rs/`, ~4.5k lines) with
- HID worker (`hidapi`): enumeration, collection/report routing, firmware feature
  report, battery attempts, notify listener, hotplug rescan;
- protocol codec with unit tests (frame layout, checksum flag, command ids, firmware
  reply/hex fallback, PEQ payloads);
- audio backend via `IPropertyStore` (C-Media names → `PROPERTYKEY`) plus Core Audio
  volume/mute, capability probing and graceful degradation; test sounds from the
  vendor WAV assets; in-memory mock backend;
- nine UI pages mirroring the original workflows (connect/home, routine, EQ with
  preset library and editors, sound effects, surround + channel test, mic, firmware,
  settings/about/log), light/dark theme, English/Portuguese/Chinese i18n, custom
  frameless window, persisted JSON config, in-app logging, and a `--probe` hardware
  validation mode;
- README documenting architecture, parity matrix, build/run and all limitations.

Screenshots of the native app: `mchose-hub-re/shots/60-native-home.png` …
`67-native-settings.png` and `70…73-native-*.png`.

**Late resolution on the audio plane (hardware-verified):** live capture of the
original client shows EQ/effect changes flowing through `audio-sdk:call` into a
C-Media AudioSDK child process, and the standard endpoint property store indeed does
not expose the vendor effect keys. A first FFI experiment with
`ConfLibInitWithGUID` enumerated 0 devices (wrong entry point). The ConfLib ABI was
then recovered from the shipped DLL + Ghidra decompilation and corrected:
`ConfLibInit()` → `CreateDeviceList()` (1 render + 1 capture device),
`GetDeviceById(type,index,DEVICEINFO*)`, and
`PropertyControl(DEVICEINFO*, name, void** slot, u64 unused, uchar action)` with
action 0 = GET (wide decimal string into a 0x400 buffer) and non-zero = SET (u32 via
the same slot). Implemented in `mchose-hub-rs/backend/confapi.rs` and verified on the
V9 PRO: EQ/effects/mic-DSP capabilities are live, the current 10-band state was read
(matching the VALORANT preset the official app had applied), and an EQ-enable
write/read-back/restore round-trip passed. Firmware feature report 170 now decodes to
`0004` (dongle, matches the official UI) / `0204` (headset). Battery remains the only
unresolved audio/device data point: the official app itself shows no battery for this
unit (`batteryLevel: null`) and every HID battery query stayed unanswered, including
the short 15-byte report the renderer sends.


**Additional parity work completed after the first delivery:**
* **C-Media DSP** — native vendor ConfLib adapter (`backend/confapi.rs`) with the
  recovered ABI; capabilities eq/effects/mic-DSP true on hardware, live band state
  read and write/read-back verified (see §12 late resolution).
* **Firmware** — feature report 170 decoded to `0004` (dongle) / `0204` (headset).
* **Microphone monitoring** — bridged to the vendor `MicListen.dll`
  (`ToggleListenDefault`/`ToggleListenByName`); capability reported and the toggle
  drives the sidecar (audible effect not acoustically verified).
* **Driver install/repair** — Settings button runs the same three
  `pnputil -i -a` INF installs as the official app (`libs\S9_PRO_14429\...`) through
  one elevated prompt.
* **Tray icon + close-to-tray + start-minimised** — native `Shell_NotifyIcon` on a
  message-only window thread, with Open/Exit menu; verified by automating WM_CLOSE
  (window hides, process stays alive) and a simulated tray click (window restores).

**Known gaps / scope decisions** (also in the README): battery level is unavailable in
the official app too (`batteryLevel: null`) and every HID query stays silent, so it is
a device limitation rather than a missing feature; virtual-7.1 writes are out of scope
(the original supports them but the user does not use this feature, and the ConfLib
control-handler SET returns an error on this unit); firmware OTA, THX runtime, cloud
account/store/RGB sync, per-app profiles, lyrics and the vendor OSD are intentionally
not reimplemented (vendor services / not needed); connect/home remains a native
approximation of the original remote iframe page.

## 15. Updated evidence index (additions)

| Evidence | Where |
| --- | --- |
| Renderer recovery (bundle, i18n, configs, script) | `mchose-hub-re/renderer/`, `findings-renderer.md` |
| Runtime CDP captures/screenshots | `mchose-hub-re/shots/`, `renderer/live/`, `captures/`, `findings-runtime.md` |
| Native exports/imports/strings/Ghidra dumps/property table | `mchose-hub-re/captures/native/`, `findings-native.md` |
| Prior-art reconstruction | `mchose-hub-re/prior-art/`, `findings-prior-art.md` |
| Native UI review against vendor spec | `mchose-hub-re/docs/review-rust-ui.md` |
| Hardware probe report | `%APPDATA%\MCHOSE_HUB_Native\probe-report.json`, `mchose-hub-rs/probe-out.json` |
| Rust sources/tests | `mchose-hub-rs/` |
