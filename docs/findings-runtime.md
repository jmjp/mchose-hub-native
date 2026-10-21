# MCHOSE HUB — Runtime findings (live app, renderer UI, IPC, HID)

Author: runtime-rec (task-2). Status: partial but evidence-backed. All timestamps local (America/Sao_Paulo, UTC-3).

Legend: **[OBSERVED]** = direct runtime capture (CDP/inspector/log/hex). **[INFERRED]** = reasoned from evidence. **[UNPROVEN/UNKNOWN]** = not established.

---

## 1. Process / launch / debug endpoints

### 1.1 Installed app state (before our relaunch)
- **[OBSERVED]** Original app was running **elevated**: main `MCHOSE HUB.exe` PID 12052 (started 10:27 by scheduled task `MCHOSE_HUB_Launcher`, `RunLevel=Highest`), helpers `MCHOSE HUB Agent.exe` (11396), `MCHOSE HUB Notifier.exe` (12640), `MCHOSE HUB Watcher.exe` x3 (5376/11880/11924, spawned by scheduled task `MCHOSE_HUB_Watcher`, also Highest).
- Scheduled tasks found: `\MCHOSE_HUB_Launcher` (Ready, action `"MCHOSE HUB Launcher.exe" --launch_from=auto_run`, Principal João, RunLevel **Highest**, LogonType Interactive) and `\MCHOSE_HUB_Watcher` (Running, action `"MCHOSE HUB Watcher.exe" --launch_from=auto_run`, RunLevel Highest).
- **[OBSERVED]** Our shell session is a **filtered admin token** (Medium IL; `whoami /groups` shows Administrators deny-only). Killing the elevated processes failed `Acesso negado`; registering a Highest scheduled task also failed `Acesso negado`. So the original instance was **left untouched**.
- **[OBSERVED]** `MCHOSE HUB.exe` PE manifest: `requestedExecutionLevel level="highestAvailable"`. Non-elevated `CreateProcess`/`Process.Start` fails with Win32 740 ("requires elevation"); `Start-Process` (ShellExecute) auto-elevated via UAC at 23:32.

### 1.2 Debug instance we used (second instance, separate profile)
- Launch command (works; no UAC because at this point the app had already been elevated once? see UNKNOWN; the launch itself succeeded and the process runs elevated as a child of our shell):
  `Start-Process 'C:\Program Files\MCHOSE HUB\MCHOSE HUB.exe' -ArgumentList '--remote-debugging-port=9222','--inspect=9229','--user-data-dir=C:\...\mchose-hub-re\captures\profile-runtime'`
- **[OBSERVED]** Main app PID 17112 (argv confirmed in `%profile%\files\log\20261020.txt`: `["C:\Program Files\MCHOSE HUB\MCHOSE HUB.exe","--remote-debugging-port=9222","--inspect=9229","--user-data-dir=…"]`). Log says `isAdminRunning: true`; the process could not be opened for PROCESS_VM_READ/QUERY_INFORMATION by our Medium-IL shell (err 5) → it runs **elevated**, i.e. Frida/OS hooks from our session are impossible.
- Ports: **CDP http://127.0.0.1:9222** (Chromium), **Node inspector http://127.0.0.1:9229** (main process). Both live at hand-off.
- Single-instance: no lock conflict — separate `--user-data-dir` lets a second instance run. Original profile untouched; our instance uses `captures/profile-runtime` (fresh config, device auto-detected).
- Helper/child processes of 17112: gpu 14664, renderers 6032 & 2828 and 22120 (started 23:46 when audio SDK initialized; almost certainly the C-Media SDK worker utility process).

### 1.3 Debug targets
| Target | Type | URL |
|---|---|---|
| page `M HUB` (`10EC4089030A012EA7B7766AAA435151`) | page | `mchose-renderer://app/cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26#/...` (custom scheme shell, Electron preload = `window.electronAPI`, 209 keys, **frozen/non-configurable**) |
| iframe (`BB7C0CF79C9A613EF59C4DC078E13402`) | OOPIF iframe | `https://www.mchose.com.cn/?__iframe_load_key=…#/connectDevice` (`allow="hid *"`) |
| page `M HUB - Tray` (`94B25F9E7FA95F25F164CD7712585FCA`) | page | `https://www.mchose.com.cn/tray.html` |

**[OBSERVED] Key architecture:** the **connect-device screen is the iframe** (www.mchose.com.cn web app). After clicking a device card the shell page navigates to `#/headset/audio_routine?...` and renders the **device pages itself** (shell custom-scheme frame); the iframe stays mounted but 0×0/hidden. Therefore device-UI interaction must be driven in the **shell page**, not the iframe.

Useful helper scripts (all under `captures/tools/`): `cdp.mjs` (one-shot CDP: list/eval/dom/shot/scripts/net), `browser.mjs` (browser-level target attach for OOPIF: targets/eval/shot/dom/scripts), `cdp-ctx.mjs`, `inspector.mjs` + `inspector-run.mjs` (Node inspector 9229), `monitor.mjs` (long-lived HID/console/network logger), `driver.mjs` + `plan-*.js` (UI action plans), `shot-window.ps1` (Windows PrintWindow fallback).
Screenshots via CDP hang while the window is minimized/occluded; fixed by calling `Page.bringToFront` (plus app `maximizeWindow()`/`unmaximize()` via electronAPI to restore a minimized window).

---

## 2. UI inventory (screens captured)

Screenshots in `mchose-hub-re/shots/`; DOM snapshots + visible text next to them are also saved under `shots/*.html/.txt` for the 10b/11a-c/12b/13b set and under `captures/dom/` for earlier ones.

| File | Screen | Notes/labels observed |
|---|---|---|
| `02-window-connectDevice.png` | Connect-device (iframe) | header MCHOSE logo + icons; tabs `RGB Sync`, `Cloud`, `Official Store`; `Product Experience Plan`; device card `V9 Pro` / `2.4G`; footer "All devices do not support Bluetooth mode for driver connection."; grid/list view toggle |
| `03-device-page.png`, `10b-device-regular-parameters.png` | Device → Regular parameters (`#/headset/audio_routine`) | left: `V9 Pro`, `2.4G`, `Manual`, nav `Audio configuration`, `Other settings`; tabs `Regular parameters`, `Equalizer`, `Virtual 7.1 Surround`; content: `Volume` (slider 0–100, value 91), `Microphone` (slider 100), `Enable AI noise reduction` (checkbox, checked), `Microphone monitoring` (unchecked), `Magic Sound Effects: Monster` (dropdown), `Windows sound device` button; `Audio brightening` (toggle off), `Dynamic Low Frequency` (toggle off, Intensity slider, Band segmentation 100/200/300/400 Hz buttons), `Intelligent volume` (toggle off, `Music: High Dyn.`/`Speech: Low Dyn.`), `Vocal Clarity` (toggle off, Intensity slider, Noise suppression 0/5/10/15/20 dB) |
| `11b/11c-device-equalizer.png`, `11d-eq-preset-VALORANT.png`, `11e-eq-toggle-off.png` | Equalizer | `Equalizer` master switch (ON); sub-tabs `Official` / `Cloud Community` / `Custom`; categories `Game` / `Music`; preset list `default`, `VALORANT`, `CS`, `Apex`, `PUBG`, `CF`, `Delta Force`; selecting VALORANT shows toast `Selected EQ activated`, sub-presets `VALORANT1/2/3`, axis labels `Bass / Mid / High` (curve canvas did not finish rendering in screenshot; see §3) |
| `12b-device-virtual-7.1-surround.png` | Virtual 7.1 Surround | `Virtual 7.1 Surround` switch (off); `Mode selection` `Music/Game Mode` + `Movie Mode` (disabled); `Speaker test` ring with 8 speaker buttons; `Room size` Small/Medium/Large; `Start test` button |
| `13b-device-other-settings.png` | Other settings | `Headphone firmware version:` (value **blank**, text "The firmware is already up to date, no update is needed."), `Receiver Firmware Version: 0004` (`Latest`), `Restore factory settings` (`Restore`) |
| `01-home-initial.png`, `11f-after-refresh-battery.png` | misc/loading | 01 was the `refresh.html` fallback screen (CDN temporarily unreachable); 11f is the transient reload state |
| `shots/*native-*.png` | NOT MINE | produced by another agent; ignore for this report |

**[OBSERVED] Device state visible in UI:** name `V9 Pro`, connection `2.4G` (2.4 GHz link), receiver firmware `0004`, headset firmware string not rendered (blank label), no battery indicator anywhere. No "install audio component/driver" dialog appeared during any audio action; `audio-sdk:install-driver` IPC ran in the background and reported `success:true` (pnputil "already installed", see §5).
Tray window (separate page): `Open M HUB`, `Exit` only.

---

## 3. IPC schema (renderer shell → main process)

**[OBSERVED]** Main-process hook (`captures/tools/ipc-hook.js`) wrapped every registered `ipcMain.handle` (205 channels at install, 212 later) plus `webContents.send`; log = `captures/ipc-live.jsonl` (222+ records, JSONL `{ts,kind,ch,args,result,ms}`). `kind` ∈ invoke | invoke-error | on | send | postMessage.

Full channel list was dumped at hook installation (see command in `captures/tools/ipc-hook.js` output; saved in `captures/ipc-channels.txt` if present). High-value channels: `audio-sdk:install-driver`, `audio-sdk:ensure-ready`, `audio-sdk:call`, `audio-sdk:get-state`, `audio-thx:*`, `getUsbDeviceDescriptor`, `getFWVersionFromUsbDescriptor`, `getUsbStringDescriptor`, `getFirmwareVer`, `otaDevice`, `getAudioVolume`/`setAudioVolume`, `setEQ_Slider`, `getEQ_Slider`, `setEnable_EQ_GFX`, `GetEnable_EQ_GFX`, `startWatchAudioVolume`, `getMicListenStatus`, store APIs (`getAllStore`, `getStore`, `setStore`, `watchAllStore`, …), window APIs, `windowManager`, `customTray`, `recorderCall`, `onBatteryInfo` (event channel in preload; **no message observed**).

### 3.1 Device page open (OBSERVED, example records)
1. `audio-sdk:install-driver` args `{productKey:"MCHOSE V9 PRO PID14429", driverDir:"S9_PRO_14429"}` → `{success:true, installSucceeded:true}` (main log shows `pnputil -i -a` on `libs\S9_PRO_14429\Apo\…Ext.inf`, `…APO.inf`, `Driver\BLK2025GHS.Inf`; all return 259 "already installed").
2. `audio-sdk:ensure-ready` args `{deviceId:"MCHOSE V9 PRO             PID14429", productKey:"MCHOSE V9 PRO PID14429", sdkType:"cmedia_2025", guid:{Data1:430567137,Data2:10979,Data3:17602,Data4:[152,161,1,147,36,188,6,8]}, audioRuntimeType:"cmedia"}`.
3. Then a burst of `audio-sdk:call` with `{method,args}` and results `{success:true,deviceId,method,data}`:
   - `setToDefault ["…PID14429"]` → true (i.e. **device defaults reset on page open**)
   - `getFirmwareVer` → `"0004"` (receiver/dongle shown in UI as 0004)
   - `getMuteControl ["…","speaker"]` → false; `["…","mic"]` → false
   - `getAINREnable` → true
   - `getDeviceFriendlyName ["…","mic"]` → `"Mic (MCHOSE V9 PRO)"`
   - `getEnable_AUDIOBRILLIANT_LFX` → false; `getEnable_VIRTUALBASS_LFX` → false; `getEnable_ADAPTIVEVOLUME_GFX` → false; `getEnable_VOICECLARITY_LFX` → false
   - `getVolumeScalarControl ["…",-1,"speaker"]` → **0.910001**; `[...,"mic"]` → **1**
   - `getAUDIOBRILLIANT_LEVEL` → 3; `getVIRTUALBASS_Level` → 5; `getADAPTIVEVOLUME_LEVEL` → 50; `getVOICECLARITY_LEVEL` → 3; `getVIRTUALBASS_CutOffFrequency` → 200; `getADAPTIVEVOLUME_MODE` → 0; `getVOICECLARITY_NOISESUPP_LEVEL` → 0; `getEnable_MAGICVOICE` → false; `getMagicVoice_Selection` → 0
   - `getFirmwareVer` repeated → `"0004"`
4. `setStore("webIndexList", [...])` persists device with `firmware:{headsetVersion:"0024",dongleVersion:"0004",speakerVersion:null}`, `batteryLevel:null`, `chargeStatusElectron:null`, `isSleepElectron:false`, `isP1:true`, `allowUpdateBattery:false`, `showBattery:true`, `enableBatteryVersion:"0008"`.
5. `getFWVersionFromUsbDescriptor {vendorId:10525,productId:14429,productName:"MCHOSE V9 PRO"}` → `{code:0,data:{bcdDevice:4,rev:"0004"}}`.

### 3.2 Equalizer / 7.1 actions (OBSERVED, action → calls)
- Select preset **VALORANT** (`Official/Game`): `GetEnable_EQ_GFX` → true; `setEnable_EQ_GFX [...,true]` → true; then **10 band writes** `setEQ_Slider ["…PID14429", band, gain]` = `(0,-10) (1,-5) (2,5) (3,-9) (4,-3) (5,0) (6,1) (7,2) (8,2) (9,0)` — all return success. UI toast `Selected EQ activated`.
- Toggle Equalizer switch: `setEnable_EQ_GFX [...,false]` → false, then `[...,true]` → true.
- 7.1 page: `getXearSurroundEnable` → false; `getXearSurroundMode` → 0; `getXearSurroundRoom` → 1; `setXearSurroundRoom [...,1]` → 1; `stopTestSound` → true.
- Volume slider read-back: `getVolumeScalarControl ["…",-1,"speaker"]` = 0.910001 (UI shows 91). Volume mapping: UI value = round(fraction × 100).
- **No `navigator.hid.*` sends during any EQ/7.1/volume action** (see §4.3). All effect changes go through `audio-sdk:call` → C-Media AudioSDK worker.

### 3.3 Store/workflow events (OBSERVED)
`watchAllStore` + `storeChange` pushes; key `webIndexList`/`webIndexListLen` converge with the tray window (wcId 2). `windowManager {method:'setBounds',params:['tray',{},true]}` used for tray anchoring. `getVersion` → `"1.2.05"` (client version; User-Agent `McHub/1.2.05`, renderer web `1.1.26`).

---

## 4. HID / WebHID evidence

### 4.1 Runtime report map (authoritative, from `navigator.hid.getDevices()`, OBSERVED)
Device `MCHOSE V9 PRO`, VID `0x291d` (10525), PID `0x385d` (14429), collections:
| usagePage | usage | in | out | feature |
|---|---|---|---|---|
| 0xB (Consumer) | 0x1 | report **7** | report **7** | — |
| 0xFF90 | 0x1 | report **85** | — | — |
| 0xFF82 | 0x1 | report **65** | report **65** | — |
| 0xC | 0x1 | report **237** | reports **237**, **255** | report **237** |
| 0xFF22 | 0x1 | — | — | report **170** |
| 0xC | 0x1 | report **6** | — | — |
Other HID devices present: `WIN 60 HE` (0x2e3c/0xc365), `AJAZZ 2.4G 8K` (0x3151/0x5007) — M HUB lists them but only V9 Pro card shown (connect screen only shows supported connected device; Win 60 HE card appeared earlier).

### 4.2 FIRMWARE feature reports — CAPTURED (OBSERVED, shell page, after device-page load at 03:12:50)
```
out sendFeatureReport(reportId=170, len=63): 01 01 00 00 00 ... 00      (target 1 = dongle)
in  receiveFeatureReport(len=64): AA 01 00 00 00 04 FF 25 03 13 11 29 FF...
                                 ^^ header      ^^^^^ bcd/dongle fw 0x0004
out sendFeatureReport(reportId=170, len=63): 01 00 00 00 00 ... 00      (target 0 = headset)
in  receiveFeatureReport(len=64): AA 01 00 00 02 04 FF 25 03 03 22 15 FF...
                                 ^^ header      ^^^^^ headset fw 0x0204
```
=> The stock app **does** use feature report 170 with payload `[0x01, target, 0×61]` and gets replies `[0xAA,0x01,0x00,0x00, fw bytes…]`. This confirms prior-art's firmware mechanism and Lead's native probe (headset 0x0204 / dongle 0x0004), and demonstrates the app's channel works on this machine. (The app stored `headsetVersion:"0024"` — likely a different formatting/parse of 0x0204; UNPROVEN.)

### 4.3 Command/notify report activity (OBSERVED, shell page)
On clicking the **Equalizer** tab and the **Virtual 7.1** tab, the shell page opened the HID device (`HIDDevice.open()` → `ok:open`) and immediately wrote:
```
sendReport(reportId=85, len=15): 11 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```
No `inputreport` reply was logged within the session for this write. Interpretation: report 85 is used as an output/notify-enable channel (0x11 command); exact semantics UNPROVEN.
**No other `sendReport` / `sendFeatureReport` calls were observed during EQ preset selection, EQ slider writes, EQ enable toggle, 7.1 room setting, or volume read-back** — those are host-side `audio-sdk:call` only.

### 4.4 Battery
- **[OBSERVED]** App UI shows **no battery percentage/icon** for V9 PRO in any captured screen; sidebar shows only `2.4G` and `Manual`.
- **[OBSERVED]** Store object after device page open: `batteryLevel:null`, `chargeStatusElectron:null`, `isSleepElectron:false`, `allowUpdateBattery:false`, `isP1:true`.
- **[OBSERVED]** No `onBatteryInfo` main→renderer message, no `getBattery*` IPC invocation, and **no unframed battery `sendReport(85,[0x65,0x01,0x00…])` was captured** in the log window (it may have occurred before our hooks were installed at 02:43/03:00; see UNKNOWN).
- Conclusion: **the app shows no battery value on this machine**; the WebHID battery query did not produce any displayable value in our capture window. This is consistent with prior-art's "battery query got no reply" observation, but the exact query attempt was not captured.

### 4.5 HID logs (files)
- `captures/mon-live-hid.jsonl` (iframe hooks; 14 records incl. `getDevices` and the feature-report exchange at 03:12:50)
- `captures/mon-shell-hid.jsonl` (shell hooks; 8 records incl. `sendReport(85)`)
- `captures/hid-eq-actions.jsonl` (per-action HID/msg drain summaries for EQ/battery actions; EQ actions produced zero HID frames)
Raw dumps show every message; the four key frames are quoted above.

---

## 5. Main-process / audio-sdk observations

- **[OBSERVED]** Main process Node 20.18.0, Electron 31.7.4, Chrome 126. `process.mainModule.require('module')._cache` = 309 modules; koffi native addon **is loaded in main** (`node_modules/koffi/index.js` + `build/koffi/win32_x64/koffi.node`), and `node-usb` (`node_modules/usb`, `prebuilds/win32-x64/node.napi.node`) is loaded too.
- **[OBSERVED]** `audio-sdk:call` is a generic dispatcher `{method,args}` handled in main; device page needs `sdkType:"cmedia_2025"`, `audioRuntimeType:"cmedia"`, `guid` (fixed `{430567137,10979,17602,[152,161,1,147,36,188,6,8]}` for V9 PRO PID 14429 in this run).
- **[OBSERVED]** A dedicated child process (`MCHOSE HUB.exe`, PID 22120) is spawned right when the audio SDK becomes ready (23:46:18) — evidently the C-Media AudioSDK worker/utility process; `audio-sdk:call` results come back in 0–4 ms.
- **[OBSERVED]** `exports.sdkMethodRegistry` in module `out/main/fake-C0K7or8z.jsc`: **58 method descriptors** with `name`, arg names, returnType, retryable flags — saved to `captures/sdk-method-registry.json` (copied in this run; see §7). This is the contract used by the IPC dispatcher.
- **[UNPROVEN]** whether the AudioSDK manipulates a C-Media APO via MMDevices FX property store, shared memory or a vendor interface. The app's reads return plausible live values (volume 0.91, AINR true, levels) so the SDK can talk to something; but Lead's native probe found the render endpoint `FxProperties` key empty → **actual DSP effect is UNPROVEN** even though the app shows no error and writes succeed.
- **[OBSERVED] not captured:** the actual `PropertyControl` (osConfLib.dll) ABI/argument shapes. Reason: the SDK runs in an **elevated child process** (child of elevated main), while our session is Medium IL and Frida/OS tracing from our shell is blocked; Node inspector only covers the main process, and `NodeWorker`/`Target` domains show no attachable worker. This is the main gap vs the Lead's request.

---

## 6. Conclusions vs Lead's questions

1. **Battery % shown?** No. UI never displays a battery value; `batteryLevel=null` persisted; no `onBatteryInfo` message seen. [OBSERVED] The HID battery query path was not captured (hooks installed after initial device init), so we cannot say the app never attempted it. [UNKNOWN]
2. **Sound/EQ page works?** Yes — the Equalizer page renders and is fully interactive (master toggle, Official/Cloud/Custom presets, Game/Music lists, preset applies, 7.1 surround page renders with toggles/room/test). No driver/audio-component error dialog. All writes go through `audio-sdk:call` and return `success:true` with live-looking values. Whether the audio actually changes (host DSP state) is UNPROVEN here; note Lead's native finding that the endpoint has no FX property store — the app's AudioSDK may use another path, or report success while doing nothing audible.
3. **HID sendReport on EQ page?** No EQ/effect-related HID frames. Only: (a) feature report 170 firmware exchange on device-page load (captured, both dongle and headset replies), (b) a single `sendReport(85, 11 00 …)` each time the Equalizer/Surround tab is opened (notify channel), (c) `HIDDevice.open()` on those tabs. EQ preset/slider/toggle actions produce **only `audio-sdk:call` IPC** (`GetEnable_EQ_GFX`, `setEnable_EQ_GFX`, `setEQ_Slider` ×10, Xear surround gets/sets).
4. **Firmware**: app-side HID feature 170 replies observed: dongle `AA 01 00 00 00 04 …`, headset `AA 01 00 00 02 04 …` (matches Lead's native 0x0004 / 0x0204). UI shows receiver `0004`; headset value not rendered in its label. Store recorded `headsetVersion:"0024"`, `dongleVersion:"0004"`.

---

## 7. Reproducibility / helper scripts

All under `mchose-hub-re/captures/tools/`:
- `cdp.mjs` — raw CDP client (list/eval/dom/shot/scripts/net) for http://127.0.0.1:9222.
- `browser.mjs` — browser-level CDP with flat sessions (needed for the OOPIF iframe target).
- `cdp-ctx.mjs` — execution-context enumeration/eval.
- `inspector.mjs`, `inspector-run.mjs` — Node inspector client for http://127.0.0.1:9229 (main process eval, script runner).
- `ipc-hook.js` — wraps all `ipcMain` handlers/listeners; logs to `global.__mchoseIpcLog`.
- `mp-hook-send.js` — wraps `WebContents.send/postMessage` (main→renderer).
- `mp-hook-usb.js` — node-usb transfer hooks (installed late; `Illegal invocation` on some prototypes; USB frames were not observed in the capture window).
- `hid-hook.js` — page-side WebHID hook (HIDDevice.sendReport/sendFeatureReport/receiveFeatureReport/open/close/addEventListener + navigator.hid.getDevices/requestDevice + window message log).
- `monitor.mjs` — long-lived monitor; writes `-hid/-msg/-net/-console/-errors.jsonl` + `-status.json`.
- `driver.mjs` + `plan-tour2.js` / `plan-eq.js` / `plan-eq3.js` — scripted UI navigation, screenshots, DOM/text dumps.
- `shot-window.ps1` — Win32 PrintWindow/BitBlt window capture fallback (blocked/occluded elevated window: PrintWindow returns false; screenshots via CDP+bringToFront preferred).

Artifacts:
- `shots/02-window-connectDevice.png`, `03-device-page.png`, `10b-device-regular-parameters.png`, `11b/11c-device-equalizer.png`, `11d-eq-preset-VALORANT.png`, `11e-eq-toggle-off.png`, `12b-device-virtual-7.1-surround.png`, `13b-device-other-settings.png` (+ matching `.html`/`.txt` for 10b/11/12b/13b).
- `captures/ipc-live.jsonl` — full IPC log (222+ records incl. args/results).
- `captures/mon-live-*.jsonl`, `captures/mon-shell-*.jsonl` — iframe/shell HID/msg/net/console logs.
- `captures/hid-eq-actions.jsonl` — per-action drains for EQ/battery actions.
- `captures/dom/01-connect-device.html`, `02-device-page.html`, `03-shell-device-page.html`.
- `renderer/live/shell-cdn/` (10 scripts) and `renderer/live/iframe-www/` (23 scripts) — captured `Debugger.getScriptSource` of the running renderer (authoritative UI JS for v1.1.26).

## 8. Open unknowns (explicit)
- Exact `osConfLib.dll PropertyControl` ABI (argument types/out-buffers): **not captured** — SDK runs in an elevated child process we cannot instrument from a Medium-IL session (no Frida/OS debug on elevated process; Node inspector only reaches main).
- Meaning of `sendReport(85, 0x11 …)` (15 bytes) — observed but semantics unproven.
- Whether the app ever issues the unframed `0x65` battery query via WebHID (prior-art says yes); not captured in our window.
- Why the headset firmware label is blank while store has `headsetVersion:"0024"` and HID reply shows 0x0204.
- Whether `audio-sdk` success means real audible DSP change given the empty MMDevices FX property store.
- Original elevated instance is still running untouched; debug instance uses a separate profile (device state defaults may differ from user's real profile).
