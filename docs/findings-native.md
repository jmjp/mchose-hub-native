# MCHOSE HUB — Native layer findings (osConfLib, helper EXEs/DLLs, THX, OTA)

Author: native-re (teammate). Scope: everything under `C:\Program Files\MCHOSE HUB\libs\` plus the
three top-level helper EXEs. Read-only analysis; nothing in the installation was modified.

Status: **delivered**. Native decompilation was done with Ghidra 12.1.4 headless (local install, see §0);
backed by the artefacts saved in `mchose-hub-re/captures/native/`.

---

## 0. Method, evidence classes, and a hard tooling limitation

Evidence classes used below:

* **OBSERVED** — read directly from the shipped bytes (PE headers, export/import tables, strings,
  `.rdata` tables, decompiled code) or from files the shipped app itself wrote (logs, JSON config).
* **OBSERVED (app binding)** — the *app's own* koffi type declarations recovered verbatim from the
  V8 bytecode constant pools (`out/main/*.jsc`). These are stronger than my inference of a signature:
  they are what the vendor told koffi to expect.
* **INFERRED** — a conclusion that spans observations (e.g. "therefore the UI talks to X").
* **UNKNOWN** — not established; listed explicitly.

### Tooling limitation (important for the Lead's report)

REA was available as an MCP server but had **no usability native-analysis provider on this Windows
host**. `binary_session` reported:

* `ghidra` → `not_configured` (`GHIDRA_INSTALL_DIR` unset in the MCP server process; JDK 21 was present),
* `hopper` → `unsupported_host` (macOS only),
* `ida` → `not_configured` (no `REA_IDA_MCP_CONFIG`),
* `kuna` → `not_configured`, and additionally `target_format_unsupported` for PE.

Therefore REA's own `open_binary` / `list_procedures` / `analyze_function` / `batch_decompile`
tools could not be used on PE targets. Target-free REA tools that *did* work and were used:
`inspect_managed_artifact`, `inspect_rust_binary` (rejected `InfLink-rs.plugin` as non-PE), `inspect_pe_resources`.
All native decompilation in this document was produced with a **locally installed Ghidra 12.1.4
headless** (`C:\rea-tools\ghidra_12.1.4_PUBLIC`, JDK 21.0.12) plus a dump script
(`C:\rea-tools\scripts\DumpAll.java`) that emits `functions.tsv`, `symbols.tsv`, `strings.tsv` and one
decompiled `.c` per function. Dumps referenced below live under `captures/native/ghidra/`.
This is a documented deviation, not an REA-provider result.

Evidence files in `captures/native/`:

| file | content |
|---|---|
| `file-inventory.json`, `pe-inventory.json` | size/sha256/PE arch/CLR flag for every native file |
| `exports-imports.json` | full export+import tables for all 14 analysed binaries |
| `version-info.json` | VS_VERSIONINFO (company, product, version, PDB-derived names) |
| `koffi-signatures.txt` | every koffi C declaration string recovered from `out/main/*.jsc` |
| `jsc-strings.json` | all printable strings from the 11 main-process bytecode caches |
| `osconflib-strings.txt`, `osconflib-utf16-strings.txt` | osConfLib strings (ASCII + UTF-16, by file offset) |
| `osconflib-imports.json` | osConfLib import table |
| `strings-*.txt` | per-binary string dumps (G9OTA, K20ProOTA, Watcher, Notifier, Agent, WindowMonitor, MicListen, smtc, thx, wyy) |
| `helper-keyword-summary.json` | keyword→strings index over the helper binaries |
| `osconflib-property-table.json`, `.tsv` | 142 recovered `name -> {PropertySet GUID, prop id}` pairs (§3.6) |
| `watcher.log`, `appdata-eq.json`, `appdata-main-log-20261020.txt` | runtime artefacts copied from `%APPDATA%\MCHOSEHUB` |
| `ghidra/{osconf,g9ota,watcher,notifier}/` | per-binary `functions.tsv`/`strings.tsv`/`symbols.tsv` + the selected decompiled functions cited in the text |

---

## 1. Component inventory

Machine-readable form: `captures/native/pe-inventory.json`, `exports-imports.json`, `version-info.json`.

| file (under `libs\` unless noted) | size | SHA-256 (short) | PE | lang | observed role | conf. |
|---|---|---|---|---|---|---|
| `osConfLib.dll` | 461 936 | `c04f998d…` | PE32+ x64 DLL | C++ (MSVC, SEH frames) | C-Media **audio configuration library** `ConfLib_Win11.dll` 1.0.8.0 — string-keyed `PropertyControl` API over KS/driver properties for EQ, virtual bass, mic DSP, surround etc. Loaded by the Electron main process through koffi. | OBSERVED |
| `DLL\G9OTA.dll` | 2 403 840 | `259478ea…` | PE32+ x64 DLL | C++ (MSVC 14, `/MDd` debug CRT) | internal name **`libdongle.dll`**; HID dongle/client enumeration + firmware OTA. Only native file that talks HID for device *data* (`HID.DLL` + `SETUPAPI`). PDB `C:\pc code 2\libdongle_dll\libdongle\x64\Debug\libdongle.pdb`. | OBSERVED |
| `DLL\K20ProOTA.dll` | 1 058 336 | `7e33f6a6…` | PE32+ x64 DLL | C++ | **"Firmware Upgrade DLL 2.0"**, internal `dll.dll`; JieLi (杰理) USB-SCSI upgrade library (`jl_usb_scsi_windows`), PDB `E:\git_109\firmware_upgrade_library_zhishizhineng\src\dll\x64\Release\dll.pdb`. | OBSERVED |
| `EXE\MCHOSE HUB Agent.exe` | 563 048 | `170c8cd1…` | PE32+ x64 EXE, GUI | C++ | 1.0.0.1, vendor 迈科技术有限公司. "LockOSD Listener": named-pipe client + RawInput registration; forwards OSD text to Notifier. | OBSERVED |
| `EXE\MCHOSE HUB Notifier.exe` | 685 928 | `245acc8d…` | PE32+ x64 EXE, GUI | C++ | 1.0.0.1. "LockOSD" server: owns `\\.\pipe\MCHOSEHUBOSD_Pipe`, renders a layered OSD window (GDI+), string `Battery low: {percent}%`. | OBSERVED |
| `MCHOSE HUB Watcher.exe` (root) | 705 896 | `13d32301…` | PE32+ x64 EXE | C++ | 1.0.0.0. HID device watchdog: enumerates HID collections from an INI-style profile list, sends a trigger output/feature report and waits for an ACK input report; writes `files\logs\watcher.log`. | OBSERVED |
| `DLL\WindowMonitor.dll` | 741 376 | `38f401f9…` | PE32+ x64 DLL | C++/MSVC | foreground-window + installed-game/app scanner: `StartFocusWatch`, `SetFocusCallbackEx(cb)`, `IA_*Json` inventory APIs; Steam/Epic/UWP/registry scanning (see strings), plus `WindowMonitorConfig.xml` (7.4 MB) game metadata next to it. | OBSERVED |
| `DLL\MicListen.dll` | 47 464 | `0f1f92a3…` | PE32+ x64 DLL | C++ | internal `toggle_listen.dll`; microphone "listen to this device" toggle (`ToggleListenById/Name/Default`), elevation check `IsProcessElevatedDll`, `GetCaptureDevicesStatusUtf8`. | OBSERVED |
| `DLL\smtc_monitor.dll` | 244 736 | `1db1009c…` | PE32+ x64 DLL | C++ WinRT | System Media Transport Controls monitor; `smtc_start(cb, pushIntervalMs, calibrationIntervalMs)`, `smtc_stop`, `smtc_request_update`; callback delivers JSON. | OBSERVED |
| `THX\thx-audio-player.dll` | 320 208 | `51a6c807…` | PE32+ x64 DLL | C++ | THX Ltd. 1.0.2.0 "Windows multichannel WAV playback (WASAPI shared mode)"; `THXAP_*`; resolves audio endpoint GUIDs by hardware id. | OBSERVED |
| `DLL\wyy.dll` | 1 297 920 | `8e^ba2da5…` | PE32+ x64 DLL | C++ (Cef/FLTK linked) | internal **`BetterNCMII.dll`** — the BetterNCM plugin loader for NetEase Cloud Music (`wyy` = 网易云). Not device related; installed by the app's "BetterNCM" integration. | OBSERVED |
| `DLL\InfLink-rs.plugin` | 1 660 699 | `f437b6ec…` | **not PE** (plugin bundle) | Rust/JS bundle | NetEase "InfLink" plugin, patches the player's `setSMTCEnable` ("This method has been patched by InfLink-rs to disable native SMTC"). Not loaded by osConfLib/koffi in the main process. | OBSERVED (strings) / INFERRED (role) |
| `*\02_APO\CmApoIce.dll` (14 variants) | 6.5–14.5 MB | see inventory | PE32+ x64 DLL | C++ | C-Media **APO effect DSP** shipped per product: `C-Media Audio Effects [CM2025]` (V9/V9_PRO/S9_PRO/G9_PRO), `[Standard]` (X9/VT008/VT009/CM002X1/K20/RzlfVjI=), `[CM002X1DRV]` (K20_PRO/K20_GT), versions 11.22–11.25. Not called by the app directly — installed into the audio engine as an APO. | OBSERVED |
| `THX\service-repair\1.0.3.0\reinstall_thx_services.exe` | 56 464 | `d5137f10…` | PE32 i386, **.NET (CLR)** | C# | THX service repair helper (managed). | OBSERVED |
| `custom_modules\audify\build\Release\audify.node` | 103 936 | `af0c8890…` | PE32+ x64 Node addon | C++/N-API | `audify` N-API audio I/O (`napi_register_module_v1`), links `opus.dll`, `rtaudio.dll` — used by the app's recorder/loopback features, not the device protocol. | OBSERVED |
| `MCHOSE HUB.exe` (root) | 181 000 704 | `45d380f2…` | PE32+ x64 Electron EXE | C++/Electron + bundled Node | Electron 31.7.4 shell; contains the embedded Node runtime strings but the app logic is `resources/app.asar`. | OBSERVED |
| `MCHOSE HUB Launcher.exe` | 524 800 | `1249c65c…` | PE32+ x64 EXE | C++ | 1.0.0.0 launcher (imports only KERNEL32/SHELL32 → spawn + elevate path). | OBSERVED |
| `resources\elevate.exe` | 107 520 | `9b1fbf0c…` | PE32 i386 EXE | C++ | generic UAC elevation helper. | INFERRED |

Not analysed in depth (out of scope by instruction): the 14 `CmApoIce.dll` builds (surveyed only),
`ffmpeg.dll`, `libGLESv2.dll`, `libEGL.dll`, `d3dcompiler_47.dll`, `vulkan-1.dll`, `vk_swiftshader.dll`,
`opus.dll`, `rtaudio.dll` — all stock third-party runtimes.

---

## 2. Architecture verdict — where the "device protocol" actually lives

This section corrects the starting hypothesis ("osConfLib.dll is likely the HID communication library").

**OBSERVED facts**

1. `osConfLib.dll` imports **no** HID/libusb/winusb API. Its full import set is:
   `WINMM.dll` (waveOut/mmio test sounds), `ADVAPI32.dll` (registry), `SETUPAPI.dll` (device-enumeration
   only: `SetupDiGetClassDevsW/EnumDeviceInterfaces/GetDeviceInterfaceDetailW/GetDeviceRegistryPropertyW`),
   `KERNEL32.dll` (incl. `DeviceIoControl`, `CreateFileW`, `LoadLibraryExW`, `GetProcAddress`),
   `USER32.dll`, `ole32.dll` (`CoCreateInstance`, `CLSIDFromString`, `InitPropVariantFromCLSID`),
   `OLEAUT32.dll`, `PROPSYS.dll` (`InitPropVariantFromCLSID`).
   Its strings are audio-only: `FaceLift=>…`, `KsProperty() FAILED!`, `GetSupportFeatureV3`,
   `SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\{Render,Capture}\`,
   `{EFX,MFX,SFX} endpoint->Activate`, `software\asio`, `usb\vid_%x&pid_%x`, `mi_00/mi_01/mi_04`, `hdaudio`.
2. HID API imports exist in exactly **three** shipped binaries:
   * `MCHOSE HUB Watcher.exe` → `HidD_GetHidGuid`, `HidD_GetPreparsedData`, `HidP_GetCaps`,
     `HidD_SetOutputReport`, `HidD_SetFeature`, `HidD_SetNumInputBuffers`, `HidD_FreePreparsedData`,
     `RegisterDeviceNotificationW` (plug/unplug + trigger/ACK only).
   * `libs\DLL\G9OTA.dll` (`libdongle.dll`) → `HidD_GetHidGuid` + `HID.DLL` + SetupAPI (enumeration and
     dongle/client OTA path).
   * nothing else (not `MCHOSE HUB.exe`, not `Agent.exe`, not `Notifier.exe`).
3. `MCHOSE HUB.exe` itself (the Electron main binary) has **no** HID/USB/SetupAPI/WinUSB import at all —
   its 17 static imports are `ffmpeg, dbghelp, OLEAUT32, WINMM, WS2_32, KERNEL32, CRYPT32, IPHLPAPI,
   WINTRUST, ntdll, USERENV, VERSION, DWrite, WINSPOOL.DRV, Secur32, WINHTTP, dhcpcsvc`. USB/HID access is
   therefore only reachable through dynamically loaded code (`node-usb`'s libusb N-API module and/or
   koffi-loaded vendor DLLs).
4. The Electron **main process** bundles `node_modules/usb` (node-usb = libusb bindings, includes
   `prebuilds\win32-x64\node.napi.node`) and exposes USB *descriptor* IPC:
   `getUsbDeviceDescriptor`, `getUsbStringDescriptor`, `getUsbVersion`, `getFWVersionFromUsbDescriptor`,
   `getFirmwareBinData` (see `captures/native/jsc-strings.json`, `index.jsc`).
5. The **UI is not shipped in `app.asar`**. `app.asar` contains only `out/main` (V8 bytecode),
   `out/preload/index.js` (plain JS, 415 lines — the IPC contract), `out/renderer/tips*` and a
   `bundled/renderer/refresh.html` fallback. The real renderer is loaded at runtime from
   `mchose-renderer://app/cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` (observed in
   `%APPDATA%\MCHOSEHUB\files\log\20261020.txt`; cached copies exist under
   `%APPDATA%\MCHOSEHUB\Cache\Cache_Data\f_*` and were already copied to `captures/cache-copy/`).
6. Firmware OTA is native: the main process loads `libs\DLL\G9OTA.dll` and `libs\DLL\K20ProOTA.dll`
   through koffi in the `out/main/workerUpgrade-*.jsc` / `workerGetDevice-*.jsc` workers.

**INFERRED conclusion**

* **Audio/APO settings** = `osConfLib.dll` (C-Media), via `PropertyControl(name, …)` over KS/driver
  properties. Not HID. Reimplementation can either FFI the vendor DLL or re-implement the KS calls
  (section 5).
* **Per-device settings (lighting, remap, DPI, battery, EQ-on-device, etc.)** are *not* implemented by
  any shipped native DLL other than the OTA/watchdog helpers. They are implemented in the
  **remote renderer bundle** (JavaScript), which reaches the device through the Electron main process'
  USB/HID bridge — the exact bridge API is an **UNKNOWN** (see §8) and is the single biggest gap for
  the reimplementation. The cached renderer bundle in `captures/cache-copy/` is the place to recover it.
* **Battery/OSD**: the renderer subscribes to the `battery_info` push event (preload
  `onBatteryInfo`); the toast is rendered by `Notifier.exe` (string `Battery low: {percent}%`), driven
  over the `MCHOSEHUBOSD_Pipe` named pipe by `Agent.exe`.

---

## 3. `osConfLib.dll` deep dive — C-Media ConfLib 1.0.8.0

### 3.1 Identity (OBSERVED)

* PE32+ x64 DLL, image base `0x180000000`, 7 sections, non-packed, no CLR, no packer section names,
  MSVC x64 with SEH (`.pdata` 0x2fb8) and `RtlVirtualUnwind`.
* `VS_VERSIONINFO`: `CompanyName=C-Media`, `LegalCopyright=Copyright (C) C-Media Electronics Inc.`,
  `FileDescription/ProductName=Audio Configuration Library for Win11`, `FileVersion=1.0.8.0`.
* PE export directory **name is `ConfLib_Win11.dll`** while `OriginalFilename` says `osConfLib.dll`
  (vendor renamed the binary for this product).
* No indication of packing/obfuscation (standard `.text/.rdata/.data/.pdata/_RDATA/.rsrc/.reloc`,
  plain assert/log strings). Ghidra analysis therefore proceeds normally.

### 3.2 Exports and exact signatures (OBSERVED export table + OBSERVED app bindings)

17 named exports, ordinal base 1 (`captures/native/exports-imports.json`). The right-hand column is the
**koffi declaration string recovered verbatim from the app's own bytecode** — it is what the shipped app
expects, so it is authoritative for the ABI:

| ord | export | RVA | app's koffi declaration (OBSERVED) | inferred meaning |
|---|---|---|---|---|
| 1 | `ConfLibInit` | `0x8db0` | *(not bound)* | legacy init without GUID filter |
| 16 | `ConfLibInitWithGUID` | `0x8d00` | `int ConfLibInitWithGUID(GUID driverInterfaceGUID)` | init ConfLib and restrict enumeration to one driver interface class GUID |
| 15 | `ConfLibUnInit` | `0xa960` | `int ConfLibUnInit()` | teardown |
| 6 | `CreateDeviceList` | `0xa690` | `int CreateDeviceList()` | enumerate render/capture devices ("FaceLift=>CreateDeviceList") |
| 5 | `GetDeviceCount` | `0xa440` | `int GetDeviceCount(int, uint*)` | count (2nd arg `uint*` out) |
| 2 | `GetDeviceById` | `0xf3d0` | `int GetDeviceById(int,int,_Out_ DEVICEINFO*)` | fetch a `DEVICEINFO` handle by index/id |
| 3 | `PropertyControl` | `0xe780` | `int PropertyControl(DEVICEINFO, wchar_t*, void**, void**, unsigned char)` | **the whole feature API** — get/set a named property (see §3.4) |
| 4 | `RegisterCallbackFunction` | `0xf3b0` | *(not bound in main)* | device-change/driver-event callback registration |
| 7/8 | `PlayTestSound`, `PlayTestSoundLoop` | `0xfbd0`,`0xfc00` | `int PlayTestSound(wchar_t *FileName, unsigned long ChannelIndex, DEVICEINFO DevInfo)` | channel test sounds through WINMM |
| 10/11 | `StopTestSound`, `CloseTestSound` | `0xfb80`,`0xfba0` | `int StopTestSound()` | stop/close test playback |
| 9 | `ShowAsioPanel` | `0x12910` | *(not bound)* | show the C-Media ASIO control panel (`software\asio` strings) |
| 17 | `EnableLogFile` | `0x13720` | *(not bound)* | debug log to file |
| 12/13 | `StopListen2Device`, `RunListen2Device` | `0x248f0`,`0x24a60` | *(not bound)* | driver event listener thread (`DeviceIoControl` loop, `DriverEventCallback`) |
| 14 | `DirectShowNotifyProc` | `0x24ac0` | *(not bound)* | DirectShow device-notification hook |

Additional native symbols recovered from bytecode that are **not** exports of `osConfLib.dll` but are
passed as *property names* through `PropertyControl`:
`GetDeviceFriendlyName`, `GetFirmwareVer`, `GetDriverVer`, `DefaultDeviceControl`, `VolumeScalarControl`,
… (full list in §3.4). This is the key structural insight: **ConfLib exposes one generic dispatcher and
the "API" is a string namespace**, not a symbol per feature.

`DEVICEINFO` is an opaque handle in the JS layer (`GetDeviceById(int,int,_Out_ DEVICEINFO*)`); the app
passes it straight into `PlayTestSound`/`PropertyControl`, and frees it with
`void DestroyDeviceInfo(void*, int)` (from the same binding block).

### 3.3 Imports and mechanism (OBSERVED)

| DLL | notable imports | what it means |
|---|---|---|
| `SETUPAPI` | `SetupDiGetClassDevsW`, `SetupDiEnumDeviceInterfaces`, `SetupDiGetDeviceInterfaceDetailW`, `SetupDiGetDeviceRegistryPropertyW`, `SetupDiEnumDeviceInfo`, `SetupDiDestroyDeviceInfoList` | enumerate KS/audio device interfaces (`ConfLibInitWithGUID(GUID)` filters by interface class GUID) |
| `KERNEL32` | `DeviceIoControl`, `CreateFileW`, `ReadFile`, `LoadLibraryExW`, `GetProcAddress`, `CreateThread`, `WaitForMultipleObjects`, `CreateEventW` | open device interface and issue driver IOCTLs; dynamic load of DirectShow/MMDevice pieces; event listener thread |
| `ole32`/`OLEAUT32`/`PROPSYS` | `CoCreateInstance`, `CLSIDFromString`, `InitPropVariantFromCLSID`, `PropVariantClear` | `IMMDeviceEnumerator`/`IPropertyStore`/`IKsControl` (MMDevice API) — the "FaceLift=>…" pipeline |
| `ADVAPI32` | `RegOpenKeyExW`, `RegQueryValueExW`, `RegCreateKeyExW`, `RegEnumKeyA` | read `MMDevices\Audio\{Render,Capture}` properties, driver descriptors, ASIO key |
| `WINMM` | `waveOutOpen/Write/Reset/Close`, `mmioOpenW/Descend/Read/Seek` | test-sound playback of WAV files |

Observed internal error-string vocabulary (OBSERVED, `captures/native/osconflib-utf16-strings.txt` and
`osconflib-strings.txt`) that maps the internal call graph:

```
FaceLift=>CreateDeviceList:: start / GetCaptureDeviceList / GetRenderDeviceList / end
FaceLift=>GetRenderDeviceList:: SetupDiGetClassDevs failed | GUID doesn't match, continue
                             | IsCMediaDevice | GetFirstIPart | TopologyTraversal | GetRenderInterface
                             | IsCMediaDevice:: KsProperty() FAILED! | GetSupportFeatureV3() FAILED!
FaceLift=>FreeDeviceList:: Free tRenderDeviceInterface->{DevVolume,AudioSession,AAPath,Extra Node 1..7}
FaceLift=>GetSupportedDeviceFormatsByAudioEngine:: IDeviceTopology/GetConnector/GetType/EnumPartsIncoming
FaceLift=>AAMuteControl|RetaskingControl|StereoMixControl|EnableDriverEventCallback
DriverEventCallback(), CreateFile() failed. hFile=0x%x, Err_Code=0x%x.
DriverEventCallback(), DeviceIoControl() failed. Error Code(0x%x)
----CCMEffect{EFX,MFX,SFX} endpoint->Activate…
```

**INFERRED**: settings are applied through (a) **KS property sets** on the C-Media audio *driver* nodes
(`IKsControl::KsProperty`, hence the `KSNODETYPE_*` strings), and (b) `IPropertyStore` on the MMDevice
for endpoint-level values; `DeviceIoControl` is used only for the driver’s event channel
(`RunListen2Device`/`DriverEventCallback`). The `CmaApoIce.dll` APO is not invoked by the app at all.

Section 3.6 below contains the resulting property registry (mechanism + 142 `PROPERTYKEY`s).

### 3.4 `PropertyControl` name table (OBSERVED, complete)

All 305 UTF-16 strings from `.rdata` are saved verbatim in
`captures/native/osconflib-utf16-strings.txt` (offset → string). The property-name block spans file
offsets `0x05682e`–`0x058d98`. Full list, grouped as the vendor groups it:

**Microphone DSP**
`gMicDRC_Enable`, `MicDRC_DecayRate`, `MicDRC_MaxGain`, `NR_Enable`, `NR_Level`, `AEC_Enable`,
`AEC_Level`, `AEC_DelayTime`, `MicArray_Enable`, `MicArray_Level`, `MicArray_MicDist`,
`MicArray_BeamWidth`, `AEC_DUMP_Enable`, `AI_NR_Enable`, `AI_MicArray_Enable`, `AI_AEC_Enable`,
`AI_MicArray_MicDist`, `AI_MicArray_BeamWidth`, `AI_NR_Level`, `AI_DUMP_Enable`, `AI_Main_Switch`,
`AI_AGC_MaxGain`, `AI_Playback_NR_Enable`, `AI_Playback_DUMP_Enable`, `AI_Playback_NR_Level`,
`AI_Playback_AGC_MaxGain`, `MicEQ_Enable`, `MicEQ_LPF_Enable`, `MicEQ_LPF_FC`, `MicEQ_HPF_Enable`,
`MicEQ_HPF_FC`, `MicEQ_PreGain`, `Enable_MICECHO`, `MICECHO_Level`, `Enable_MICARRAY_LFX`,
`MICARRAY_BEAMWIDTH`, `Enable_AEC_LFX`, `MicPositionControl`, `ENC_SELFCALIB`, `Enable_ENC_LFX`,
`Enable_NOISEREDUCTION_LFX`, `NoiseReduction_Level`

**Speaker EQ / bass management / virtual bass**
`Enable_EQ_GFX`, `EQ_Slider1` … `EQ_Slider10`, `Enable_BM_GFX`, `BM_Slider_Frequency`,
`BM_LargeSpeaker`, `BM_Slider_BassLevel`, `SpeakerDelay_C`, `SpeakerDelay_S`, `SpeakerDelay_B`,
`Enable_VIRTUALBASS_LFX`, `VIRTUALBASS_Mode`, `VIRTUALBASS_Level`, `VIRTUALBASS_CutOffFrequency`,
`HWEQBassControl`, `HWEQTrebleControl`, `VrmsLimiter_{Enable,Level,Attack,Release,PreGain}`

**Surround / spatial / speaker geometry**
`Enable_SURRSENS_LFX`, `Enable_SURRSENS_USEHP`, `SURRSENS_EXPFACTOR`, `ENABLE_NEOPC_FOR_SURRSENS`,
`ENABLE_SURRSENS_BASSENHANCEMENT`, `SURRSENS_BASSENHANCEMENT_GAIN`, `ENABLE_SURRSENS_DIALOGCLARITY`,
`SURRSENS_DIALOGCLARITY_GAIN`, `Enable_NEOPC_LFX`, `NEOPC_SLIDER_DIMENSION`, `NEOPC_SLIDER_CENTERGAIN`,
`NEOPC_MODE`, `SURRSENS_CONTENTTYPE`, `SURRSENS_SPEAKERANGLE`, `SURRSENS_MIXLFE`,
`Enable_SURRMAX_FOR_HPSPK`, `Enable_Channel_COPY_LFX`, `Enable_SS_LFX`,
`SS_{LF,C,RF,RS,RB,LB,LS,LFE}_{ANGLE,DIST}`, `Enable_SwapLfeCn_LFX`, `Enable_SwapSrBk_LFX`,
`CONLAB_DIPOLE_{SPK,SRC,ALPHA}`, `Enable_CONLAB_HPSPK_LFX`, `CONLAB_HPSPK_{USEHP,ROOMSIZE,SFX,SPKANGLE,MODE}`,
`SpeakerShifterControl`, `VirtualSurroundEffectControl`, `XearSurround71Control`, `IsSupportXearSurround71`

**Head tracking**
`HeadTracking_Enable`, `HeadTracking_Algorithm`, `HeadTracking_Position{Front,Right,Top}`,
`HeadTracking_Orientation{Yaw,Pitch,Roll}`, `HeadTracking_RoomSize`, `HeadTracking_ReverbEnable`,
`HeadTracking_ReverbConfig`

**Voice/level effects**
`Enable_VOCALCANCEL_GFX`, `VOCALCANCEL_LEVEL`, `Enable_KEYSHIFT_GFX`, `KEYSHIFT_LEVEL`,
`Enable_RFX_GFX`, `RFX_ENVIRONMENT`, `RFX_ROOMSIZE`, `Enable_SMART_VOLUME_GFX`,
`Enable_ADAPTIVEVOLUME_GFX`, `ADAPTIVEVOLUME_LEVEL`, `ADAPTIVEVOLUME_MODE`,
`Enable_VOICECLARITY_LFX`, `VOICECLARITY_LEVEL`, `VOICECLARITY_NOISESUPP_LEVEL`,
`Enable_AUDIOBRILLIANT_LFX`, `AUDIOBRILLIANT_LEVEL`, `Enable_VOCALEFFECT_LFX`, `VOCALEFFECT_LEVEL`,
`Enable_MAGICVOICE`, `MagicVoice_Selection`, `Enable_UltraPCIIPlus`, `Enable_SonicMapper`

**DTS**
`DTSUPCII_{Bass,S2,LEQ,Boost,Symmetry,ADE,Enable...}` (`_Enable`/`_Level`/`_Mode`/`_LF_Level`/`_HF_Level`/
`_VoiceGain`/`_StageExpansion`), `DTSUPCII_GetDefaultPropertiesForProfile`, `DTSUPCII_HWVol_Control`,
`DTSAPO4_Enable`, `DTSAPO4_GetOpModeList`, `DTSAPO4_OpModeControl`, `DTSInteractiveControl`,
`IsSupportDTSInteractive`

**Endpoint / device / volume**
`VolumeControl`, `AAVolumeControl`, `VolumeScalarControl`, `AAVolumeScalarControl`,
`VolumeScalarControlEx`, `AAVolumeControlEx`, `AAVolumeScalarControlEx`, `MuteControl`, `AAMuteControl`,
`AAMuteControlEx`, `GetMaxVol`, `GetMinVol`, `GetVolStep`, `GetAAMaxVol(Ex)`, `GetAAMinVol(Ex)`,
`GetAAVolStep(Ex)`, `GetAANum`, `GetAAInfo`, `GetVolumeChannelCount`, `DefaultDeviceControl`,
`EndpointEnableControl`, `DevFormatControl`, `DevChannelNumControl`, `DevSampleRateControl`,
`DevBitDepthControl`, `DevChanMaskControl`, `OemChannelNumControl`, `OemSampleRateControl`,
`OemBitDepthControl`, `OemChanMaskControl`, `GetChannelPeakMeter`, `GetPeakMeter`,
`GetMeterChannelCount`, `SpectrumControl`, `GetSpectrumData`, `IsSupportFormat`, `IsShareADC`,
`ADCSelectorControl`, `AmplifierControl`, `HPAmplifierControl`, `BoostControl`, `AGCControl`,
`EXControl`, `EnableEX`, `GFSControl`, `EnableGFS`, `DeviceSpecificControl`,
`OutputTerminalTypeControl`, `ThirdPartyFeatureControl`, `SoundRadarControl`, `MixWithWaveControl`,
`RecordingPreviewControl`, `HWENCControl`, `RetaskingControl`, `StereoMixControl`, `MicPositionControl`,
`StartDummyCaptureStream`, `EnableDriverEventCallback`

**Informational getters**
`GetDeviceFriendlyName`, `GetDeviceDesc`, `GetDeviceID`, `GetSymbolicLink`, `GetExtraInfo`,
`GetFirmwareVer`, `GetDriverVer`, `GetDirectXVer`, `GetAudioCodecName`, `GetSupportFeature`,
`GetSpeakerShiftVolumeLevel`, `GetSpeakerShiftDistanceByDB`, `ASUS_U3Plus_GetSpectrumControl`

> Note: `GetFirmwareVer` here is the **driver/APO** firmware version, not the headset MCU version. The
> headset firmware version comes from the USB descriptor (`getFWVersionFromUsbDescriptor`) or from the
> device's own HID protocol.

### 3.5 UI IPC → `PropertyControl` name mapping (OBSERVED, `out/preload/index.js` + bytecode)

| preload IPC | property name(s) used | note |
|---|---|---|
| `GetEnable_EQ_GFX` / `setEnable_EQ_GFX` | `Enable_EQ_GFX` | bool |
| `getEQ_Slider(index)` / `setEQ_Slider(index,value)` | `EQ_Slider{1..10}` | 10 bands, integer dB-ish (-13…+14 observed in `eq.json`) |
| `getEnable_VIRTUALBASS_LFX`, `getVIRTUALBASS_Level`, `getVIRTUALBASS_CutOffFrequency` | `Enable_VIRTUALBASS_LFX`, `VIRTUALBASS_Level`, `VIRTUALBASS_CutOffFrequency` | |
| `getEnable_ADAPTIVEVOLUME_GFX`, `…_LEVEL`, `…_MODE` | `Enable_ADAPTIVEVOLUME_GFX`, `ADAPTIVEVOLUME_LEVEL`, `ADAPTIVEVOLUME_MODE` | |
| `getEnable_VOICECLARITY_LFX`, `…_LEVEL`, `…_NOISESUPP_LEVEL` | `Enable_VOICECLARITY_LFX`, `VOICECLARITY_LEVEL`, `VOICECLARITY_NOISESUPP_LEVEL` | |
| `getEnable_AUDIOBRILLIANT_LFX`, `getAUDIOBRILLIANT_LEVEL` | `Enable_AUDIOBRILLIANT_LFX`, `AUDIOBRILLIANT_LEVEL` | |
| `getEnable_Channel_COPY_LFX` | `Enable_Channel_COPY_LFX` | |
| `getAINREnable`, `getNR_Enable` | `AI_NR_Enable`, `NR_Enable` | |
| `getEnable_MAGICVOICE`, `getMagicVoice_Selection` | `Enable_MAGICVOICE`, `MagicVoice_Selection` | |
| `getXearSurroundEnable/Room/Mode` | via `XearSurround71Control` | handled by the `XearSurround71Control` control handler, not by the PROPERTYKEY table |
| `getVolumeScalarControl(channel,type)` / `set…`, `getMuteControl(type)` | `VolumeScalarControl`, `MuteControl` (+`AA…` for app-volume) | |
| `setToDefault` | per-feature defaults; bytecode has `#handleSetToDefault` | |
| `playTestSound(value)` / `stopTestSound` | `PlayTestSound(wchar_t* file, ulong channel, DEVICEINFO)` / `StopTestSound()` | WAV per channel |
| `getFirmwareVer` | `GetFirmwareVer` | *driver* version |
| `getDeviceFriendlyName(type)` | `GetDeviceFriendlyName` | |

The app's own binding block additionally proves which calls the **main** process makes directly:
`ConfLibInitWithGUID`, `ConfLibUnInit`, `CreateDeviceList`, `GetDeviceCount`, `GetDeviceById`,
`PropertyControl`, `PlayTestSound`, `StopTestSound`, `DestroyDeviceInfo`, `GetDeviceFriendlyName`,
`GetFirmwareVer` — i.e. everything else in preload is an IPC name, not a DLL export.

### 3.6 The property registry — mechanism and 142 recovered `PROPERTYKEY`s (OBSERVED)

**Mechanism (decompiled, high confidence).** `PropertyControl` is a two-level, string-keyed dispatcher.
It converts the caller's `wchar_t*` name into a `std::wstring`, hashes it with **FNV-1a (64-bit)**
(offset basis `0xcbf29ce484222325`, prime `0x100000001b3`, over the UTF-16 bytes) and looks it up in
`std::unordered_map<std::wstring, Entry>` tables that are populated by static initializers
(`FUN_180003ea0`, `FUN_180001000`, `FUN_180002f90`).

Two entry shapes exist:

1. **`PROPERTYKEY` entries** — `Entry{ …, PROPERTYKEY key; }` at entry+0x30 (`GUID` = +0x30..0x3F,
   `DWORD pid` = +0x40). Executed by `FUN_18000f8f0` (0x18000f8f0):

   ```
   GET : PROPVARIANT v; v.vt = VT_UI4;
         store->GetValue(&key, &v)          // store vtable +0x28
         swprintf(outBuffer /*0x200 wchars*/, L"%d", v.ulVal)
   SET : PROPVARIANT v; v.vt = VT_UI4; v.ulVal = *(uint32_t*)*in;
         store->SetValue(&key, &v); store->Commit?   // vtable +0x30 / +0x58 path
   ```

   So **values cross the API as 32-bit integers**, and `PropertyControl` GET returns them as a
   decimal wide string (`%d`) in a 0x400-byte caller buffer (`param_3` must be `void**` pointing at
   that buffer; the function zeroes 0x400 bytes first). SET reads a `uint32_t` through `param_4`.
   The store is an **`IPropertyStore`** on the audio device/endpoint (vtable `GetValue`/`SetValue`,
   `PROPVARIANT` + `PropVariantClear`), selected by two tags: device type (render/capture — the entry
   struct carries it) and a group tag (`0`, `0x1000`, `0x2000`, `0x3000`) that picks one of three
   per-device property-store members (`device+0x110/0x118/0x120` for render,
   `+0x140/0x148/0x150` for capture; `0x3000` falls back to the global store at `DAT_18006d418`).
   Before a GET the library also writes `PROPERTYKEY {1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E}, pid 5`
   (the standard `PKEY_AudioEndpoint_*` family, "disable system effects") with value 1 — i.e. it
   ensures the APO/effects chain is active.

   > This is *not* `IKsControl`/`KSPROPERTY`; it is the documented COM property store. **A pure-Rust
   > implementation is therefore possible**: `IMMDeviceEnumerator → IMMDevice::OpenPropertyStore →
   > IPropertyStore::GetValue/SetValue` with the `PROPERTYKEY`s below, via `windows-rs`.

2. **Control entries** — second table (`FUN_180005c80`): name → C++ handler function pointer for the
   FaceLift "controls" (`VolumeScalarControl`, `MuteControl`, `AAVolumeControl`, `DefaultDeviceControl`,
   `DevFormatControl`, `DevSampleRateControl`, `GetPeakMeter`, `SpectrumControl`, `XearSurround71Control`,
   `DTSInteractiveControl`, `GetFirmwareVer`, `GetDriverVer`, `GetDirectXVer`, `GetAudioCodecName`,
   `EndpointEnableControl`, `IsSupportFormat`, `GetChannelPeakMeter`, `AmplifierControl`, `EXControl`,
   `GFSControl`, `ADCSelectorControl`, `BoostControl`, `AGCControl`, `HPAmplifierControl`,
   `DeviceSpecificControl`, `OutputTerminalTypeControl`, `ThirdPartyFeatureControl`, `MixWithWaveControl`,
   `RecordingPreviewControl`, `HWENCControl`, `VirtualSurroundEffectControl`, `SoundRadarControl`,
   `MicPositionControl`, `RetaskingControl`, `StereoMixControl`, `EnableDriverEventCallback`,
   `StartDummyCaptureStream`, `SpeakerShifterControl`, `GetDeviceFriendlyName/Desc/ID/SymbolicLink/ExtraInfo`,
   `GetVolumeChannelCount`, `GetMaxVol/GetMinVol/GetVolStep`, `GetAA*`, `GetAANum/GetAAInfo`, …).
   These are implemented with MMDevice/`IAudioEndpointVolume`/`IDeviceTopology`/KS calls, i.e. they map
   onto documented Windows APIs (see §5).

**Recovered `PROPERTYKEY` table.** 142 unique `name → {PropertySet GUID, property id}` pairs were
extracted from the initializers by resolving the referenced `.rdata` constants
(`captures/native/osconflib-property-table.json` and `.tsv`). 23 vendor property sets:

| PropertySet GUID | props | names |
|---|---|---|
| `{BD341FFB-9FD1-4ED4-8159-7999D546EA8A}` | 11 | `Enable_EQ_GFX` (id 0), `EQ_Slider1..10` (id 1..10) |
| `{5E96E4D9-FF2C-4D84-9BD6-FE10CF815009}` | 12 | `Enable_AUDIOBRILLIANT_LFX` (0), `AUDIOBRILLIANT_LEVEL` (1), `Enable_VOICECLARITY_LFX` (2), `VOICECLARITY_LEVEL` (3), `VOICECLARITY_NOISESUPP_LEVEL` (4), `Enable_VIRTUALBASS_LFX` (5), `VIRTUALBASS_Mode` (6), `VIRTUALBASS_Level` (7), `VIRTUALBASS_CutOffFrequency` (8), `Enable_ADAPTIVEVOLUME_GFX` (9), `ADAPTIVEVOLUME_LEVEL` (10=`0xa`), `ADAPTIVEVOLUME_MODE` (11=`0xb`) |
| `{5860EE50-9705-436A-96DB-BB18A130E08B}` | 7 | `Enable_BM_GFX`(0), `BM_Slider_Frequency`(1), `BM_LargeSpeaker`(2), `SpeakerDelay_C`(3), `SpeakerDelay_S`(4), `SpeakerDelay_B`(5), `BM_Slider_BassLevel`(6) |
| `{F9736E6B-5342-4690-82B7-95E78DF992A0}` | 17 | `Enable_SS_LFX`(0), `SS_{LF,C,RF,RS,RB,LB,LS,LFE}_ANGLE`(1..7? ids 1-7), `SS_{LF,C,RF,RS,RB,LB,LS,LFE}_DIST`(8..15=`0x8..0xf`), `SS_LFE_ANGLE`=`0x10` |
| `{3BDB362E-2722-4CF3-BF07-5F44BF350FC9}` | 11 | `Enable_SURRSENS_LFX`(0), `Enable_SURRSENS_USEHP`(1), `SURRSENS_EXPFACTOR`(2), `ENABLE_NEOPC_FOR_SURRSENS`(3), `ENABLE_SURRSENS_BASSENHANCEMENT`(4), `SURRSENS_BASSENHANCEMENT_GAIN`(5), `ENABLE_SURRSENS_DIALOGCLARITY`(6), `SURRSENS_DIALOGCLARITY_GAIN`(7), `SURRSENS_CONTENTTYPE`(8), `SURRSENS_SPEAKERANGLE`(9), `SURRSENS_MIXLFE`(10) |
| `{B1132012-70C8-49CF-B860-E34ADA163170}` | 10 | `NR_Enable`(0), `NR_Level`(1), `AEC_Enable`(2), `AEC_Level`(3), `AEC_DelayTime`(4), `MicArray_Enable`(5), `MicArray_Level`(6), `MicArray_MicDist`(7), `MicArray_BeamWidth`(8), `AEC_DUMP_Enable`(9) |
| `{D9D2C54D-289D-4BF2-8BEF-3270C977AE33}` | 9 | `AI_Main_Switch`(0), `AI_NR_Enable`(1), `AI_AEC_Enable`(2), `AI_MicArray_Enable`(3), `AI_MicArray_MicDist`(4), `AI_MicArray_BeamWidth`(5), `AI_NR_Level`(6), `AI_DUMP_Enable`(7), `AI_AGC_MaxGain`(8) |
| `{8B7C7133-B9D4-4ADA-BC89-E23220EAE4B3}` | 4 | `AI_Playback_NR_Enable`(0), `_NR_Level`(1), `_AGC_MaxGain`(2), `_DUMP_Enable`(3) |
| `{9306F425-13AE-4078-8147-1EA5CB55CBF1}` | 6 | `MicEQ_Enable`(0), `MicEQ_LPF_Enable`(1), `MicEQ_LPF_FC`(2), `MicEQ_HPF_Enable`(3), `MicEQ_HPF_FC`(4), `MicEQ_PreGain`(5) |
| `{540CEEEB-FC13-4001-87DA-4E9FE93B4990}` | 3 | `MicDRC_Enable`(0), `MicDRC_DecayRate`(1), `MicDRC_MaxGain`(2) |
| `{A9334B7E-12FD-4918-B565-461073332B91}` | 6 | `Enable_NOISEREDUCTION_LFX`(0), `Enable_ENC_LFX`(1), `ENC_SELFCALIB`(2), `Enable_MICARRAY_LFX`(3), `MICARRAY_BEAMWIDTH`(4), `Enable_AEC_LFX`(5) |
| `{3BC6329A-2397-4FF3-92F5-2CD9894BB889}` | 4 | `Enable_MICECHO`(0), `MICECHO_Level`(1), `Enable_MAGICVOICE`(2), `MagicVoice_Selection`(3) |
| `{64805010-C7E7-4408-9BE0-658ED5DD0ED2}` | 4 | `Enable_VOCALCANCEL_GFX`(0), `VOCALCANCEL_LEVEL`(1), `Enable_KEYSHIFT_GFX`(2), `KEYSHIFT_LEVEL`(3) |
| `{D77F6FDC-386F-44B5-9278-70B636002B3D}` | 4 | `Enable_NEOPC_LFX`(0), `NEOPC_SLIDER_DIMENSION`(1), `NEOPC_SLIDER_CENTERGAIN`(2), `NEOPC_MODE`(3) |
| `{85546CD9-8513-4FCD-BFBF-289AE49FBBBB}` | 11 | `HeadTracking_Enable`(0) … `_ReverbConfig`(10) (see TSV for each id) |
| `{C020CDBF-756F-4B6D-9031-BE6E20CF1C9F}` | 10 | `Enable_CONLAB_HPSPK_LFX`(0), `CONLAB_HPSPK_{USEHP,ROOMSIZE,SFX,SPKANGLE,MODE}`(1..5), `CONLAB_DIPOLE_{SPK,SRC,ALPHA}`(5..7), `Enable_SURRMAX_FOR_HPSPK`(8) |
| `{9287D038-9DDE-4472-9509-D1EE9371C1D6}` | 5 | `VrmsLimiter_{Enable,Level,Attack,Release,PreGain}`(0..4) |
| `{686AD5B3-9A96-4723-87BC-EBCD95B407DE}` | 3 | `Enable_RFX_GFX`(0), `RFX_ENVIRONMENT`(1), `RFX_ROOMSIZE`(2) |
| `{CD1455A5-D63C-4830-91CD-206DA24E10F1}` | 1 | `Speaker_Channel_Setting`(0) |
| `{31AF2432-4E37-47C5-9785-9D9529E07129}` | 1 | `Enable_Channel_COPY_LFX`(0) |
| `{1D7D6D22-151D-49AC-A999-EFF4CAA5178D}` | 1 | `Enable_SwapLfeCn_LFX`(0) |
| `{3A053648-49C9-4FD5-8500-CF595DDB08E2}` | 1 | `Enable_SwapSrBk_LFX`(0) |
| `{E34C0B43-5F18-49B3-96ED-3F07E8028F69}` | 1 | `Enable_SMART_VOLUME_GFX`(0) |

Per-name id assignments, including the ones omitted above for brevity, are in
`captures/native/osconflib-property-table.tsv`.

**Not in the PROPERTYKEY table** (they go through the control table instead, i.e. documented Windows
APIs rather than APO properties): `VolumeScalarControl`, `MuteControl`, `AAVolumeControl`,
`XearSurround71Control`, `DTSInteractiveControl`/`DTSAPO4_*`/`DTSUPCII_*`, `GetFirmwareVer`,
`GetDriverVer`, `GetPeakMeter`/`SpectrumControl`, all `Get*Info` getters.

**Value encoding caveat (UNKNOWN, bounded).** `FUN_18000f8f0` always builds a `VT_UI4` PROPVARIANT and
marshals via `%d`, so the APO properties are integer-typed at this ABI. Whether a given property is a
bool (0/1), an enum, or a scaled float (e.g. EQ gain in dB, level 0-100) is decided by the *renderer's*
own descriptor table in the app's bytecode (`_getFloatProperty`, `_getSpeakerIntegerProperty`,
`_getMicBooleanProperty`, … in `fake-*.jsc`/`index.jsc`) — recovering that table is a JS-side task and
is the remaining gap for exact UI parity (see §8 probe 4).

---

## 4. Helper processes and DLLs

### 4.1 `MCHOSE HUB Watcher.exe` — HID watchdog with a *configurable* trigger/ACK protocol

This is the most directly useful native finding for the HID side. **OBSERVED** strings
(`captures/native/strings-MCHOSE_HUB_Watcher.exe.txt`):

```
[device.0]  [device.1]  [device.2]  [device.3]     <- INI-style profile sections
reportid=00
reportlength=64
triggerreportid=C1
ackreportid=C2
ackusagepage=0001
ackusage=0080
ackinterface=mi_02&col04
runEntryName=MCHOSE HUB Watcher
```

API imports: `HidD_GetHidGuid`, `HidD_GetPreparsedData`, `HidP_GetCaps`, `HidD_FreePreparsedData`,
`HidD_SetOutputReport`, `HidD_SetFeature`, `HidD_SetNumInputBuffers`, `RegisterDeviceNotificationW`,
`UnregisterDeviceNotification`, plus SetupAPI enumeration.
Named kernel objects (single-instance guard + stop signal):
`Local\MCHOSEHUB_DeviceWatcher_{Guardian,State,StopRequested,Watchdog,Worker}`.
Log: `%APPDATA%\MCHOSEHUB\files\logs\watcher.log` (path string `\watcher.log`).
PDB: `e:\code\windowsproject\MHubDeviceWatcher\x64\Release\MCHOSE HUB Watcher.pdb`.

Runtime log lines (**OBSERVED**, `captures/native/watcher.log`):

```
HID scan complete, active sessions=0
No read endpoint for 3837_3020_ep0
No HID session for VID=3837 PID=3020 read=0x0001/0x0000 ack=0x0001/0x0080 (mi_02&col04). Check device connection.
Open ACK channel failed on col04:
Open ACK fallback channel failed on mi_01:
No ACK endpoint for …
ACK failed for …
read/ack ready     ack=0x…
```

**Interpretation (INFERRED, high confidence).** Per device profile the watcher keeps two HID
collections: a *read* collection with usage page/usage `0x0001/0x0000` (generic desktop / undefined)
and an *ack* collection `0x0001/0x0080`, both on `mi_02&col04`; it writes a trigger report with id
`0xC1` (`HidD_SetOutputReport`/`HidD_SetFeature`) and waits for an input report id `0xC2`. Reports are
64 bytes; the plain data report uses report id `0x00`. Profiles present in this build cover
**VID `0x3837`** with PIDs `0x3020`, `0x302D`, `0x3030`, `0x303E` (MCHOSE 2.4 GHz dongle/keyboard
family) — **not** the V9 PRO headset (VID `0x291D`, PID `0x385D`), which is why the log shows
`active sessions=0` on this machine.

**Decompiled profile template (OBSERVED, `captures/native/ghidra/watcher/1400026b0_FUN_1400026b0.c`).**
The watchdog's profile is *not* read from an INI on this build — a template is constructed in code and
pushed into the session vector (`vector<DeviceProfile>` at session+0x50). Values observed:

```
VID  = 0x3837                    (constant in code; PID comes from the caller)
read iface  = L"mi_01"           (fallback read endpoint)
ack  iface  = L"mi_02&col04"
ack usagePage/usage = 0x0001 / 0x0080     (packed 0x00800001)
trigger report id  = 0xC1        (constant 0xC1010001 packed region)
ack     report id  = 0xC2
report length      = 0x40 = 64
```

**What the trigger is for (OBSERVED strings, high confidence).** The ACK is not telemetry: the
watcher compares two payload bytes and, on match, launches/focuses the Hub:

```
validatelaunchpayload, launchpayloadbyte0, launchpayloadbyte1   <- profile keys
Trigger <...> -> launch Hub            "Launched Hub" / "Hub restore via re-launch" /
"Hub focus via re-launch" / "Hub restore posted to UI thread" / "Hub exe not found: " /
"Launch failed, code=%Id, target=%s" / "chrome_widgetwin" / "mchose" / "runas" / "open"
```

I.e. **that HID `0xC1`→`0xC2` exchange is the hardware "open the Hub" hotkey**, not battery. The
launch payload bytes are compared against `launchpayloadbyte0/1`, and the process is started
(`MCHOSE HUB.exe`, optionally elevated/`runas`, debounced by `launchdebouncems`) or the existing
window is restored/focused (`chrome_widgetwin` class).

**Full configuration schema (OBSERVED strings, all keys present in the binary):**

```
global / [device.N]:
  hubexe, launchelevated, launchdebouncems, autostart, guardianrestartms, runentryname, name
  interface, usagepage, usage                 (read endpoint)
  ackusagepage, ackusage, ackinterface        (ack endpoint)
  reportid, triggeroffset, triggerreportid, ackreportid, reportlength
  validatelaunchpayload, launchpayloadbyte0, launchpayloadbyte1
watchdog plumbing:
  --watchdog / --worker / --guardian  (self re-exec modes)
  format "%04x_%04x_%s" -> "3837_3020_ep0" (log ids)
  "SetupDiGetClassDevsW failed, err=", "No read endpoint for", "No ACK endpoint for",
  "Invalid read length", "Opened ... PID=", "read/ack ready", "ACK failed for <...>"
report write path (OBSERVED, 1400082a0): overlapped WriteFile first; if that fails or the handle is
absent, falls back to HidD_SetOutputReport, then HidD_SetFeature.
```

### 4.2 `MCHOSE HUB Notifier.exe` — OSD server + low-battery toast

**OBSERVED** strings/imports: `CreateNamedPipeW`, `ConnectNamedPipe`, `DisconnectNamedPipe`,
`PeekNamedPipe`, `CreateThread(PipeServer)`, `CreateWindowEx(OSD)`, `wRegisterClass(OSD)`,
`GDI+`/`gdiplus.dll`, `WINMM`; singleton `Local\LockOSD_Server_Singleton_{F25B3F08-1C2A-43D9-9D25-3B36A04B0C81}`;
pipe `\\.\pipe\MCHOSEHUBOSD_Pipe`; log strings `[LockOSD][Srv] animIn done -> start hold`,
`Battery low: {percent}%`; PDB `\AgentAndOsd\LockOSD\x64\Release\MCHOSE HUB Notifier.pdb`.
So: layered, animated OSD window rendered by GDI+, fed by a named-pipe server; battery-low is one of
the OSD text templates.

**Pipe wire format (OBSERVED, `captures/native/ghidra/notifier/140009600_FUN_140009600.c`).**
Server created as
`CreateNamedPipeW(L"\\\\.\\pipe\\MCHOSEHUBOSD_Pipe", mode=3, maxInstances=10, outBuf=0x1000, inBuf=0x1000, timeout=0, sa)`
with an explicit SDDL `D:(A;;GA;;;BA)(A;;GA;;;SY)(A;;GRGW;;;BU)S:(ML;;NW;;;LW)` — admins/SYSTEM full
control, BUILTIN Users read/write, mandatory label *low*, i.e. deliberately writable from the
non-elevated UI. The accept loop is `PeekNamedPipe` + `ReadFile` into a 0x1000 buffer; a message is
accepted only if

```
len >= 0x10                        (rejected otherwise)
u32[0] == 0x4C4F5344               ('LOSD' multichar constant = magic)
u32[1] == 2  (and != 1)            command/type;  len must be >= 0x14 for this command
```

and for `type == 2` the first 20 bytes are copied into a heap block and
`PostMessageW(osdHwnd, 0x80C8 /* WM_APP+0xC8 */, 0, block)` is posted to the OSD window. Other
`type` values are read and dropped by this handler, so the *text* carrier (extra bytes beyond the
20-byte header, or a different message/type handled elsewhere) is **UNKNOWN** — the Agent side
(`SendTextV2(type, on, ...)`) is the counterpart to probe.

### 4.3 `MCHOSE HUB Agent.exe` — OSD pipe client

**OBSERVED** strings: `Local\LockOSD_Listener_Singleton_{7D1778C8-37E2-4B9C-A2A8-3F1B3B4C21B2}`,
`LockOSD Listener`, `LockOSD_Sink`, `RegisterRawInputDevices`, `WaitNamedPipeW`,
`[LockOSD] SendTextV2 open FAILED: type=%u on=0x%08X err=%lu`,
`[LockOSD] SendTextV2 open wait FAILED: …`, PDB `\AgentAndOsd\LockOSD\x64\Release\MCHOSE HUB Agent.pdb`.
So: Agent owns raw input + listens and calls `SendTextV2(type, …)` over the pipe; `type` is an OSD
message-kind enum, `on` is a second parameter (likely a screen/monitor handle).
Main process starts it as `WinAgentManager`.

### 4.4 `G9OTA.dll` (`libdongle.dll`) — the only native HID data path

**OBSERVED** export table (`captures/native/exports-imports.json`):

| ord | export | RVA | app's koffi declaration (OBSERVED) |
|---|---|---|---|
| 1 | `CreateExecutorHandle` | `0x66c2` | `void* CreateExecutorHandle(char*)` |
| 2 | `DestroyDeviceInfo` | `0x8599` | `void DestroyDeviceInfo(void*, int)` |
| 3 | `DestroyExecutorHandle` | `0x5b50` | `void DestroyExecutorHandle(void*)` |
| 4 | `FreeString` | `0x6fb4` | *(not bound)* |
| 5 | `getUtf16` | `0x74be` | *(not bound)* |
| 6 | `iterAllHidDevice` | `0x2ce3` | `int iterAllHidDevice(void*)` |
| 7 | `ota_update_client` | `0x161d` | `ota_update_client` (worker binding) |
| 8 | `ota_update_dongle` | `0x1519` | `ota_update_dongle` |
| 9 | `ota_update_dongle_single_post` | `0x4458` | `ota_update_dongle_single_post` |
| 10 | `read_online_device` | `0x7284` | `read_online_device` (worker binding) |
| 11 | `runAllTask` | `0x7e14` | `void runAllTask(void*)` |

Imports: `KERNEL32`, `SETUPAPI`, `HID.DLL` only. Debug CRT (`VCRUNTIME140D`), PDB path
`C:\pc code 2\libdongle_dll\libdongle\x64\Debug\libdongle.pdb`, and the debug library name
`libdongle.dll` — i.e. **this is a non-release build shipped inside the product**.

Worker-side structures named in the bytecode (OBSERVED): `dongle_callback_t`, `dongle_client_info_t`
with fields `channel_seq`, `support_ota`, `mac_addr`, `device_name`; `ota_upgrade_callback_t` with
events `on_start_ota`, `on_stop_ota`, `on_cancel_ota`, `on_error_ota`, `on_finish_ota`; `UsbDevInfo`.
Worker flow constants: `devicePath`, `dllPath`, `firmwareData`, `getDevices`.

**Decompiled findings (OBSERVED; evidence in `captures/native/ghidra/g9ota/`).**

*Export thunks* all forward to real bodies (jmp rel32):
`CreateExecutorHandle`→`0x180016220`, `runAllTask`→`0x1800163a0`,
`DestroyExecutorHandle`→`0x1800163e0`, `read_online_device`→`0x180040cd0`,
`ota_update_client`→`0x180040d50`, `ota_update_dongle`→`0x180040e20`,
`ota_update_dongle_single_post`→`0x180040f00`, `iterAllHidDevice`→`0x180061780`,
`DestroyDeviceInfo`→`0x180061ae0`, `FreeString`→`0x18006b260`, `getUtf16`→`0x18006b110`.

*HID transport (0x18006a5d0 sync, 0x18006a950 async, 0x1800691b0 / 0x180069440 read):*

```
write:  buf = zeroed 0x41 (65) bytes; buf[0] = 0x00 (report id 0);
        memcpy(buf+1, chunk, n) where n = min(remaining, 0x40);
        WriteFile(hid, buf, 0x41, ..., &ov)     // chunked over the whole payload, 64 B per report
read :  ReadFile(hid, userBuf, userLen, ..., &ov) with a dedicated event
both :  overlapped I/O; ERROR_IO_PENDING = 0x3E5; CancelIoEx on wait timeout
```

*Enumeration (0x1800616f0 → 0x180061b80):* `HidD_GetHidGuid(&guid)` then
`SetupDiGetClassDevsW(&guid, NULL, NULL, 0x12 /* PRESENT|DEVICEINTERFACE */)`,
`SetupDiEnumDeviceInfo` + `SetupDiEnumDeviceInterfaces`, `SetupDiGetDeviceInterfaceDetailW` for the
device path, plus `SetupDiGetDeviceRegistryPropertyW` for SPDRP_* strings. `iterAllHidDevice` fills a
caller array of 0x18-byte records `{char* path; char* name; char* other;}`.
**There is no VID/PID filter and no `vid_`/`pid_` string inside G9OTA.dll** (verified by string scan and
by absence of constants) — device selection is done by the JavaScript worker after enumeration.

*Task model:* the library is a Boost.Asio-style async pipeline (`io_context`, `libdongle::Task`,
`libdongle::IOLoop`, `ReadDataPackage`/`WriteDataRequest` queues, `libdongle::RcspCommand`). Public
functions are thin: copy the caller's args into a task, `post` it, and hand the result to a
callback/`AsyncInfo`. `read_online_device(handle, void* devInfo32)` copies **32 bytes** of device info,
submits a task and stores the result on the handle object; `ota_update_*` copy a **64-byte frame**
(`param_7`, exactly one HID report payload) plus a progress callback (`on_start/stop/cancel/error/finish_ota`).
`RcspCommand` shows the dongle command layer is the JieLi **RCSP** protocol family, but the concrete
opcodes/CRC of the frames were not recovered from this build in the time available (UNKNOWN; the
`ReadDataPackage`/`WriteDataRequest` framing code is the place to continue).

### 4.5 `K20ProOTA.dll` — JieLi USB-SCSI firmware upgrade

**OBSERVED**: internal `dll.dll`, "Firmware Upgrade DLL 2.0", C++ class
`.?AVjl_usb_scsi_windows@@` (JieLi USB-SCSI transport), imports only `KERNEL32`+`ADVAPI32`
(`DeviceIoControl`, `CreateFileW`), PDB `E:\git_109\firmware_upgrade_library_zhishizhineng\src\dll\x64\Release\dll.pdb`
("zhishizhineng" = 至知智能). 17 exports, with the app's koffi signatures (OBSERVED):

```
int32_t JL_queryDevicePidVid(char, char*, uint32_t, char*, uint32_t)
int32_t JL_queryDevicePidVidByPath(char*, char*, uint32_t, char*, uint32_t)
int32_t JL_queryDeviceDesc(char, char*, uint32_t)
int32_t JL_queryDeviceDescByPath(char*, char*, uint32_t)
void    JL_getChipDeviceName(char*, uint32_t)
int32_t JL_loadUfwData(const char*, uint32_t)      // firmware image load
int32_t JL_getUfwPidVid(char*, uint32_t, char*, uint32_t)
int32_t JL_getUfwCrc(uint16_t*, uint32_t*)         // expected CRC of loaded image
int32_t JL_getUfwDataCrc(uint16_t*, uint32_t*)     // CRC of data
int32_t JL_upgradeDevice(char, int, int, JL_UPGRADE_PROGRESS_CALLBACK*)
int32_t JL_upgradeDeviceByPath(char*, int, int, JL_UPGRADE_PROGRESS_CALLBACK*)
void    JL_UPGRADE_PROGRESS_CALLBACK(uint32_t)     // progress 0..100
void    JL_addPcModeName(char*) / JL_clearPcModeName() / JL_setPcModeWaitTime(uint32_t)
int32_t JL_setDualAppMode(...) / JL_setDualUbootMode(...) / JL_setResetWaitTime(uint32_t)
```

So K20 Pro/K20 GT firmware is updated over the chip's USB-SCSI (mass-storage) channel, with the
library doing the DFU handshake; the app only supplies the `.ufw`/bin blob and a progress callback.

### 4.6 `WindowMonitor.dll` — foreground-app/game detection

**OBSERVED** exports with app bindings:

```
void __stdcall StartFocusWatch(int intervalMs)
void __stdcall StopFocusWatch()
void __stdcall SetFocusCallbackEx(FocusCallback *cb)
void __stdcall FocusCallback(const char* name, uint32_t pid, const char* path)
int __stdcall IA_GetAppsJsonAll(str16 search, _Out_ char *outBuf, int cbOut)
int __stdcall IA_GetGameDetailsByPathJson(str exePath, _Out_ char *outBuf, int cbOut)
int __stdcall IA_RunMultiPlatformGameScanJson(string configFile, _Out_ char *outBuf, int cbOut)
IA_FindGamesByFile / IA_FindGamesJson / IA_GetAppsJson / IA_RefreshCache / RunMultiPlatformGameScan
```

Strings show registry/launcher scanning (`HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`,
`AppModel\Repository\Packages`, Steam `libraryfolders`, `LauncherInstalled.dat`, Epic/Uplay, SQLite stub)
and it consumes `libs\DLL\WindowMonitorConfig.xml` (7.4 MB game database).
Role: tells the renderer which app/game is in the foreground, so per-app lighting/EQ profiles apply.
Loaded by main via koffi (`StartFocusWatch`/`SetFocusCallbackEx`), pushing `onActiveAppChange`.

### 4.7 `MicListen.dll` (`toggle_listen.dll`)

**OBSERVED** exports + app bindings:
`int IsProcessElevatedDll()`, `int ToggleListenDefault(int)`,
`int ToggleListenById(int, const char*, const char*)`, `int ToggleListenByName(int, const char*, const char*)`,
`int GetCaptureDevicesStatusUtf8(char*, int)`; imports only `ole32/ADVAPI32/KERNEL32/MSVC`.
This is the "microphone monitoring/side-tone" switch (Windows "Listen to this device"), toggled per
capture endpoint; it does not touch the device's hardware sidetone.

### 4.8 `smtc_monitor.dll`

**OBSERVED** exports: `int smtc_start(SmtcCallback *cb, int pushIntervalMs, int calibrationIntervalMs)`,
`void smtc_stop()`, `void smtc_request_update()`, callback `void SmtcCallback(const char* json)`.
WinRT (`api-ms-win-core-winrt-*`) + `OLEAUT32`. Delivers now-playing metadata (title/artist/app) as JSON
to the main process, which forwards it to the renderer (`onLyricsMediaUpdate`) and to the device's
display (through the unknown device bridge).

### 4.9 THX audio stack

`libs\THX\thx-audio-player.dll` (THX Ltd. 1.0.2.0, "Windows multichannel WAV playback (WASAPI shared mode)"):
exports `THXAP_Init`, `THXAP_Shutdown`, `THXAP_PlayWav(str16 guid, str16 wav, int layout)`,
`THXAP_Pause/Resume/Stop/Seek(double)`, `THXAP_GetState`, `THXAP_GetDurationSeconds`,
`THXAP_GetPositionSeconds`, `THXAP_IsFormatSupported`, `THXAP_LayoutChannelCount/Mask`,
`THXAP_ListProducts`, `THXAP_GetEndpointsJson`, `THXAP_ResolveEndpointGuid`,
`THXAP_ResolveEndpointGuidByHwid`, `THXAP_LastErrorMessage`.
Imports `AVRT` (MMCSS real-time audio), `SETUPAPI`, `ole32`, `SHELL32`. Used for THX-branded test/demo
playback (e.g. the 7.1 test video) onto a specific endpoint.

THX gRPC protos are plain text in the install (both read in full):

`libs\THX\proto\thx_audio_properties.proto` (v1.1.0, 2026-03-19) — service **`thx.proto.ThxAudioProperties`**:

| RPC | in → out |
|---|---|
| `GetEndpointList` / `GetEndpoint` | endpoint ids / `Endpoint{id,description,active}` |
| `GetPropertyList`, `GetPropertyDefinition`, `GetProperty` | `Property{id,type,float/int/bool/string/string_array/float_array}`, `PropertyDefinition{…,is_volatile,validator}` |
| `SetProperty` | → `Empty` |
| `SubscribeToPropertyChanges` | → `stream Property` |
| `GetPresetGroupList`, `GetPresetGroupDefinition`, `GetPresetList`, `SelectPreset`, `GetSelectedPreset`, `SavePreset`, `ResetPreset`, `ResetPresetAllowed`, `AddPreset`, `RemovePreset`, `GetPresetName/Description`, `ExportPreset`/`ImportPreset`/`ParsePreset`, `SubscribeToPresetChanges`, `SubscribeToEndpointChanges`, `GetVersion` | preset CRUD + change streams |

Property types: `PropertyType_{None,Bool,Int,Float,String,StringArray,FloatArray,Event}`;
preset JSON is the interchange format (`ExportPreset → preset_json`).

`libs\THX\proto\thx_client_shell.proto` — service **`thx.THXClientShellService`** (csharp_namespace `Thx`):
`Initialize(InitializeRequest{client_type,flags}) → InitializeResponse{success,session_id}`,
`Uninitialize`, `GetParameter/SetParameter/GetParametersArray/SetParametersArray` over
`ParameterValue{oneof double|int64|bool|string|bytes}` with an `int32 parameter_id`,
`GetPayload/SetPayload` over `int32 payload_id` + `bytes` (payload_type enum lives in the vendor's
`thx_monitor_params.h`, not shipped), and `RegisterChangeNotification →
stream ChangeNotification{type: PARAMETER_CHANGED|PAYLOAD_CHANGED|EVENT_CHANGED, param1, param2, payload}`
where `payload` carries e.g. the audio endpoint id.

IPC names in preload (`AudioTHX*`) show the main process implements this client, including
`AudioTHXCheckAfunix` (AF_UNIX socket transport on Windows) and installer download/repair flows.

### 4.10 `wyy.dll` and `InfLink-rs.plugin` (media integrations, not device protocol)

`wyy.dll` = `BetterNCMII.dll`, a BetterNCM build for NetEase Cloud Music (imports `libcef.dll`, FLTK,
`dwmapi`; strings show the BetterNCM plugin manager, `betterncm_native.app.auto_update`, plugin market).
`InfLink-rs.plugin` is the InfLink NetEase plugin (JS/Rust bundle; not PE) whose patched method strings
(`This method has been patched by InfLink-rs to disable native SMTC`) confirm the app uses it to feed
now-playing data. Both are installed/uninstalled from the UI (`installBetterNcm`, `removeInfLinkPlugin`).
They are irrelevant to the HID/audio protocols but explain the "lyrics"/"now playing" features.

---

## 5. Audio stack and pure-Rust feasibility

### 5.1 Who implements what (OBSERVED)

APO builds shipped, one directory per product family (`libs\<PRODUCT>\02_APO\CmApoIce.dll`). Content
hashing shows only **6 distinct builds** for 14 product folders:

| build sha256[:12] | size | FileDescription / FileVersion | product folders |
|---|---|---|---|
| `ac636154fc58` | 14 424 064 | `C-Media Audio Effects [CM2025]` 11.22582.24.1219 | `G9_PRO`, `S9_PRO`, `V9`, `V9_PRO` |
| `3bd03e3275b8` | 14 423 552 | same family (unique build) | `S9_PRO_14429` |
| `7b7da3a4023e` | 14 461 952 | `C-Media Audio Effects [Standard]` 11.23485.25.0318 | `X9` |
| `fee10914a0a8` | 12 288 512 | `C-Media Audio Effects [Standard]` 11.25525.25.0923 | `VT008`, `VT009` |
| `be37238ed1b3` | 7 591 936 | `[CM002X1DRV]` family | `CM002X1`, `K20`, `RzlfVjI=` (base64 for `G9_V2`) |
| `ffa38a1704ea` | 7 626 752 | | `G9_WIRED` |
| `3e2f099c0cc7` | 6 572 544 | `[CM002X1DRV]` 11.25317.25.0828 | `K20_PRO` |
| `c97a673e869d` | 6 606 848 | | `K20_GT` |

(That is 8 rows for 8 distinct hashes across 14 folders; versions quoted are the ones recovered by full
VS_VERSIONINFO parse.) The product→APO mapping is data-driven — the folder name is the product key used
by the renderer/app config (note the obfuscated `RzlfVjI=` folder name).

| layer | component | controlled by |
|---|---|---|
| DSP effects (EQ, virtual bass, surround, mic NR/AEC, magic voice, …) | `CmApoIce.dll` (C-Media APO, per-product build) installed into the Windows audio engine | the **driver/APO property store**, written by `osConfLib!PropertyControl` via KS properties |
| endpoint volume/mute/format/meters/spectrum | C-Media audio driver + MMDevice API | `osConfLib` (`VolumeScalarControl`, `GetPeakMeter`, `SpectrumControl`, `DevFormatControl`, …) |
| THX spatial / vendor preset service | `THX*` services (gRPC over AF_UNIX) + `thx-audio-player.dll` | app's `AudioTHX*` IPC → gRPC |
| test sounds | `osConfLib!PlayTestSound` (WINMM) and `thx-audio-player` (WASAPI/AVRT) | renderer |

### 5.2 Reimplementation options (INFERRED, concrete)

1. **FFI the vendor DLL (lowest risk, works today).** Load `libs\osConfLib.dll` with `libloading`,
   declare the 11 functions the app uses (`ConfLibInitWithGUID`, `CreateDeviceList`, `GetDeviceCount`,
   `GetDeviceById`, `PropertyControl`, `PlayTestSound`, `StopTestSound`, `DestroyDeviceInfo`,
   `GetDeviceFriendlyName`, `GetFirmwareVer`, `ConfLibUnInit`), and drive the same property-name strings
   from §3.4. `PropertyControl`'s `void**` parameters are in/out variants of the value; the exact
   marshalling for each property is the `%d`/`VT_UI4` scheme in §3.6 and can be
   recovered empirically with a 3-line probe (call with a null in/out and read the returned error code).
   Requires a C-Media driver installed (the app's own `AudioSDKInstallDriver` path shows this is expected).
2. **Direct property-store calls (no vendor DLL) — RECOMMENDED.** Exactly what §3.6 documents:
   `IMMDeviceEnumerator::GetDefaultAudioEndpoint`/`EnumAudioEndpoints` → `IMMDevice::OpenPropertyStore` →
   `IPropertyStore::GetValue/SetValue` with the recovered `PROPERTYKEY`s, values as `VT_UI4`
   (`windows-rs`: `Win32::Media::Audio::{IMMDeviceEnumerator, IPropertyStore}` +
   `Win32::UI::Shell::PropertiesSystem::{PROPERTYKEY, PROPVARIANT}`). No vendor private IOCTL is involved,
   so this is portable across machines that have the C-Media driver installed. Caveats:
   the *which store* selection (§3.6 `0`/`0x1000`/`0x2000`) may matter for a few properties, and
   GET/SET via `osConfLib` returns `%d` text, so the caller must parse/format integers itself.
3. **Pure Windows APIs only (no C-Media-specific anything).** Honest capability list:
   * endpoint volume/mute: `IAudioEndpointVolume` — works;
   * per-app volume: `ISimpleAudioVolume`/`IAudioSessionControl2` — works;
   * endpoint format: `IAudioClient::IsFormatSupported`/`Initialize` — works;
   * peak meters: `IAudioMeterInformation` — works (per-endpoint only, not per-channel pre-APO);
   * **everything EQ/DSP/surround/mic-processing: impossible without the C-Media property GUIDs** —
     these are APO-private properties, not standard Windows audio APIs.

**Conclusion for the Lead:** 100 %-native (no vendor DLL) is *possible* in principle for the audio
panel **iff** §3.6 yields the property GUID/ids; otherwise the practical plan is
(a) FFI `osConfLib.dll` for effects, (b) pure Windows APIs for volume/format/meters. Both paths are
compatible with an egui UI that simply replaces the renderer.

---

## 6. Firmware / OTA

| product family | path | mechanism | evidence |
|---|---|---|---|
| G9 / G9 PRO / dongle-based families | `libs\DLL\G9OTA.dll` (`libdongle.dll`) | HID: enumerate HID devices (`iterAllHidDevice`, SetupAPI+HidD_GetHidGuid), read device info (`read_online_device`, `dongle_client_info_t{channel_seq,support_ota,mac_addr,device_name}`), then `ota_update_client` (wired/client) or `ota_update_dongle`/`ota_update_dongle_single_post` (through the 2.4 GHz dongle) with progress callbacks `on_start_ota/on_stop_ota/on_cancel_ota/on_error_ota/on_finish_ota` | export table, koffi strings, imports |
| K20 / K20 GT (JieLi chipset) | `libs\DLL\K20ProOTA.dll` | USB **SCSI** (`jl_usb_scsi_windows`): `JL_queryDevicePidVid*` to find the device, `JL_loadUfwData(file,len)` + `JL_getUfwCrc` to validate the image, `JL_upgradeDevice*` with `JL_UPGRADE_PROGRESS_CALLBACK(uint32 progress)`; `JL_setDualAppMode`/`JL_setDualUbootMode`/`JL_setResetWaitTime`/`JL_setPcModeWaitTime` control the boot-mode dance | exports, koffi strings, PDB name |
| headset (V9 PRO etc.) | **UNKNOWN** | the shipped OTA DLLs target dongles/JieLi; a V9 PRO update path was not observed in static artefacts (`onOtaMessage`/`otaDevice` IPC exists, worker selection is by product config) | — |

Firmware blob formats: K20 expects the JieLi `.ufw` container (library computes/validates CRC and
PID/VID from the loaded image); G9OTA's expected blob format was not established (the worker just passes
`firmwareData` + `devicePath`). Verification is done inside the vendor DLLs — the app never checks the
image itself (INFERRED from the worker's thin surface).

---

## 7. Prioritised protocol summary for the reimplementation

| # | feature | implementation chain (as far as established) | confidence | what still needs runtime confirmation |
|---|---|---|---|---|
| 1 | Audio effects panel (EQ, bass, surround, mic DSP, magic voice, voice clarity, adaptive volume, audio brilliant, channel copy) | renderer → IPC → main → `osConfLib!PropertyControl(DEVICEINFO, L"<name>", void**, void**, u8)` → KS/driver property → APO | **OBSERVED** chain + property GUID/id (§3.6); UNKNOWN only the per-property value scale | value encoding/ranges per property; whether an unprivileged process can set them |
| 2 | Volume/mute (endpoint + app volume) | same `PropertyControl` names `VolumeScalarControl`/`MuteControl`/`AA…`, or plain `IAudioEndpointVolume` | OBSERVED names | channel/type argument semantics |
| 3 | Device enumeration for audio | `ConfLibInitWithGUID(driverGUID)` + `CreateDeviceList` + `GetDeviceCount/ById` (returns `DEVICEINFO`), matched to USB `usb\vid_%x&pid_%x` + `mi_0x` strings | OBSERVED | the concrete interface GUID(s) per product |
| 4 | Firmware OTA (dongle families) | `G9OTA.dll` HID path | OBSERVED exports + transport (65-byte reports, report id 0); UNKNOWN RCSP frame opcodes/CRC | report ids/sizes, CRC, packet sequencing |
| 5 | Firmware OTA (K20) | `K20ProOTA.dll` JieLi USB-SCSI | OBSERVED | `.ufw` layout, SCSI command bytes |
| 6 | Device presence/keep-alive for 2.4 GHz devices | `Watcher.exe`: HID trigger report `0xC1` → ACK input report `0xC2`, usage page/usage `0x0001/0x0000` (read) and `0x0001/0x0080` (ack), `mi_02&col04` (+`mi_01` fallback), 64-byte reports, VID `0x3837`, PID from profile | OBSERVED config/template/API/log; ACK payload semantics UNKNOWN | 64-byte ACK content (the watcher only compares 2 payload bytes = launch payload); does the device also send battery there? |
| 6b | **Hardware "open Hub" hotkey** | the same `0xC1`→`0xC2` exchange: watcher validates `launchpayloadbyte0/1` then launches `MCHOSE HUB.exe` (optionally elevated, debounced) or restores/focuses the existing window (`chrome_widgetwin`) | OBSERVED | exact trigger payload written in report `0xC1` (offset/key `triggeroffset`), and the real ACK bytes for the V9 PRO |
| 7 | Battery % + low-battery toast | renderer ← `battery_info` push from main; OSD text via `MCHOSEHUBOSD_Pipe` handled by `Notifier.exe` (`Battery low: {percent}%`) | OBSERVED components; **source of the % is UNKNOWN** — not `Watcher.exe` (no battery strings/logic), not `G9OTA` (`dongle_client_info_t` = {channel_seq, support_ota, mac_addr, device_name}), not `Notifier` (renderer only) | must come from the renderer's device protocol (live HID capture) or a USB/vendor command in main |
| 8 | Foreground app/game profiles | `WindowMonitor.dll` `StartFocusWatch` + `FocusCallback(name,pid,path)` → `onActiveAppChange` | OBSERVED | — |
| 9 | Now-playing / lyrics | `smtc_monitor.dll` (+ BetterNCM/InfLink for NetEase) → `onLyricsMediaUpdate` | OBSERVED | how metadata reaches the device display |
| 10 | Mic monitoring toggle | `MicListen.dll` `ToggleListen{Default,ById,ByName}` | OBSERVED | — |
| 11 | THX spatial presets/test | `AudioTHX*` IPC → gRPC `thx.proto.ThxAudioProperties` / `thx.THXClientShellService` + `thx-audio-player.dll` | OBSERVED | THX service install/repair semantics |
| 12 | Lighting / remap / DPI / device-side EQ / dongle pairing | **not in any shipped native DLL** → remote renderer JS through the main-process USB bridge | INFERRED | **highest-priority gap**; see §8 |

---

## 8. Limitations and next probes

**Limitations**

1. No native analysis provider in REA on this host → decompilation came from locally installed Ghidra
   (documented in §0). REA Evidence IDs therefore do not exist for the native dumps; the raw Ghidra
   output is retained under `captures/native/ghidra/`.
2. The **device command layer for settings** is not present in the shipped native binaries. The renderer
   is remote (`cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26`); only its HTTP-cache copy
   (`captures/cache-copy/`) and IndexedDB are available locally. Any claim about lighting/remap/DPI
   command bytes must come from that bundle or from live capture — not from static native analysis.
3. `watcher.log` on this machine shows only VID `0x3837` profiles and `active sessions=0`; the V9 PRO
   (VID `0x291D`, PID `0x385D`) never appears, so I have **no observed V9 PRO frame**.
4. Deep-dive of the 14 `CmApoIce.dll` builds was intentionally limited to version/identity survey.
5. `THX` payload/parameter id enums live in a header (`thx_monitor_params.h`) that is not shipped.

**Next probes (ranked)**

1. Live HID capture with the V9 PRO attached while the app's UI adjusts: battery polling, lighting,
   EQ-on-device, sidetone, 7.1 — record `bSetupPacket`/`wValue`/report ids and 64-byte payloads.
   The G9OTA transport shape (report id 0, 65-byte writes, 64-byte payloads) is a good prior for the
   renderer path too.
2. Capture the Watcher protocol on a VID `0x3837` device: log the `0xC1` trigger (what is at
   `triggeroffset`) and the full 64-byte `0xC2` ACK, then compare bytes to `launchpayloadbyte0/1`.
3. Extract the renderer bundle from `captures/cache-copy/` (`f_00000d`, `f_00000e`, `f_000013`,
   `f_000028` are the big JS chunks) and diff against the loaded page to find the HID/USB bridge API.
4. Runtime: attach to `MCHOSE HUB.exe` main process (CDP already in use by another teammate) and
   enumerate `electronAPI` + the WebSocket/IPC calls that carry device frames.
5. For per-property KS/PROPERTYKEY semantics: instrument `osConfLib!PropertyControl` (e.g. via Frida)
   and log `(name, PROPERTYKEY, value)` for one UI session — that converts §3.6 from static extraction
   into measured evidence and also yields the JS-side type/scale table.
6. For G9OTA's dongle protocol: continue from `libdongle::RcspCommand` / `ReadDataPackage` /
   `WriteDataRequest` (functions around `0x1800xxxxx` in the 15 037-function inventory) to recover
   RCSP opcodes, sequence numbers and the checksum used in the 64-byte OTA frames.

**What was definitively ruled out** (useful to avoid dead ends)

* `osConfLib.dll` does **not** touch HID or libusb (import table has neither `HID.DLL` nor any USB API).
* `MCHOSE HUB Agent.exe` / `MCHOSE HUB Notifier.exe` are OSD-only (pipe + GDI+ + RawInput); no HID.
* `wyy.dll` (BetterNCMII) and `InfLink-rs.plugin` are NetEase Cloud Music integrations, unrelated to
  the device.
* `WindowMonitor.dll` is foreground-window/game scanning only.
* There is no VID/PID table inside `G9OTA.dll` — device selection happens in JavaScript.

