# MCHOSE HUB — Renderer / UI Layer Findings

Status: **v2 (2026-10-21)** — recovered renderer identified, downloaded, decoded and documented.
`[VERIFIED]` = confirmed against artifacts/hashes/live CDN; `[INFERRED]` = consistent evidence, not
directly executed; `[UNKNOWN]` = not yet recovered.

Author: renderer-rec (shared task `task-1`).
Scope: the Electron **renderer** (UI) of MCHOSE HUB — pages, navigation, controls, wording, data
schemas (`electronAPI`, store keys, remote config), and renderer-side device I/O (WebHID).
Native main-process internals (`.jsc` bytecode, `osConfLib.dll`, THX gRPC) are other teammates'
lanes; referenced here only for context.

---

## 0. TL;DR — what a Rust/egui reimplementation needs to know

1. **The UI is a remote Vue 3 SPA, not shipped in the asar.** The Electron window loads
   `https://cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` (`<title>M HUB</title>`).
   This is *different* from the browser product at `https://www.mchose.com.cn/` (web driver
   v3.16.32). `[VERIFIED: byte-identical to the Chromium cache of the installed app]`
2. **Two layout generations per device** drive different audio UI:
   - *legacy* (`isNewLayout` absent) → **AudioSDK / C-Media** runtime, 10-band EQ, Xear virtual 7.1,
     LFX/GFX effect API, `audio_eq`/`audio_surround`/`audio_mode` pages, `routineLegacy` landing page.
   - *new layout* (`isNewLayout: true`) → **THX** runtime + on-device PEQ over WebHID, plus
     `audio_speaker` / `audio_mic` / `audio_surround_thx` pages and `routineCommonParams`.
   The connected **MCHOSE V9 PRO (VID 0x291D/10525, PID 0x385D/14429) is `cmedia` + legacy**.
3. **Device I/O in the renderer is WebHID** (`navigator.hid`, `sendReport`/`inputreport`) using a
   63-byte binary frame (0xAA start, XOR checksum) documented in §10. Audio DSP settings do *not* go
   over HID on legacy devices — they go through the main process
   (`AudioSDKCall(method,[deviceId,...args])` or `AudioTHXCall({productKey,method,args})`).
4. The UI is one device proxy (`p.get/p.set(methodName,args)`) plus a **56-case adapter** that maps
   legacy C-Media names to THX properties — so one UI drives both runtimes (§7.6b, machine-readable
   at `renderer/notes/audio-method-map.json`).
5. Shell = fixed header (`commonHeader`) + left sidebar + content. Only 4 top-level routes;
   **`/index` (the device list/connect "home") is itself an iframe of the browser web driver**
   (`urlIndex` = `www.mchose.com.cn/#/connectDevice`), so the native UI is just theme + headset
   pages plus an iframe shell (§3, §4.1).
6. All UI text is i18n-driven with 9 locales; the desktop bundle's complete string table was
   recovered (`renderer/i18n-prod/*.json`), and the web driver's too (`renderer/i18n/*.json`).

---

## 1. Component identity and version facts `[VERIFIED]`

Source of truth: `https://cdn.mchose.com.cn/configCenter/config/globalLoader.js`
(`MC_CONFIG_GLOBAL_PROD_LOADER`; embedded `activeValue`, re-fetchable from
`https://cdn.mchose.com.cn/configCenter/config/global.json`).

| Field | Value |
|---|---|
| `pcVersion.tag` | `1.2.05` — hash `6c6b1a93c1cba5736bab8b7142dd0afb9b15a5ce`, publishTime `1788579929861` |
| `webVersion.tag` | `v3.16.32` — branch `HEAD`, hash `64d504f7455be3632764da31b155775d796f36da`, publishTime `1792500198866` |
| `rendererVersion.tag` | `1.1.26` — hash `c3e0aef66e61ad53af997e7ccd95c7772301e38d`, publishTime `1792469197799` |
| `refreshTime` | `10` |
| installed renderer version (store) | same: `1.1.26` / `c3e0aef6…` |

Local copies:
- `renderer/site/configCenter-activeValue.json` — decoded global config (keys: `customFields`
  {`headerBtnConfigs`, `expertViewConfig`, `bgConfigV2`, `bgConfig`, `gtConfig`, `isRemoteLang`,
  `firstGuide`}, `version`, `pcVersion`, `webVersion`, `rendererVersion`, `refreshTime`).
- `renderer/site/configCenter/config/globalLoader.js` — loader template + embedded JSON.

Page titles: Electron renderer `<title>M HUB</title>`; browser web driver
`<title>M HUB 迈从科技 官方驱动</title>`.

---

## 2. Evidence sources and method

### 2.1 Live network artifacts (browser UA; saved under `renderer/site/` and `renderer/prod/`)

| Artifact | URL | Notes |
|---|---|---|
| **Electron renderer entry** | `https://cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` | 1 159 B; loads `./assets/main-xA7mzCYt.js`, `main-Bigq7yvw.css`, `./assets/_plugin-vue_export-helper-Ce2ZIww_.js`, plus remote `configCenter/static/files/echarts.min_313ee9e3ab27.js` and `configCenter/pcPublic/publicResources/lottie.min.js` |
| Renderer asset graph (64 files) | `https://cdn.mchose.com.cn/customPage/prod/assets/*` | BFS to closure → `renderer/prod/` + `renderer/prod/_crawl.json` (sha256 per file) |
| Browser web driver | `https://www.mchose.com.cn/` | 6 952 B; `main-DeNJg3f5.js` + `theme-IdBKOUK0.js` (2.3 MB, i18n+vendor) + `purify.es-CVNN4l_y.js` (7.7 MB app) → `renderer/site/` |
| Global remote config | `https://cdn.mchose.com.cn/configCenter/config/globalLoader.js` | 29 069 B; embedded JSON + reload logic to `…/config/global.json` |
| Asset host | `https://cdn.mchose.com.cn/configCenter/...`, `…/customPage/iconfont/iconfont.css` | used by both apps |

Key renderer chunks (`customPage/prod/assets/`):

| chunk | size | role (observed) |
|---|---|---|
| `main-xA7mzCYt.js` | 1 146 875 | app shell: hash router (4 top-level routes), Pinia stores, `commonHeader`, updater UI, **audio runtime managers + AudioSDK→THX adapter**, device-config table, firmware minimum table, OTA plumbing, home (`/index`) component |
| `_plugin-vue_export-helper-Ce2ZIww_.js` | 1 055 685 | shared lib: vue-i18n **all 9 locale tables**, StoreClient (`getStore/setStore/watchAllStore` envelopes), env-config fallback (`Ft`/`Fh`), Element Plus, utils |
| `headsetPage-DNy5Z21r.js` | 2 495 877 | headset device page: sidebar, battery (HidEQ 0x0B), AudioSDK/THX install dialog, bundled pdf.js manual viewer, OTA |
| `index-Cbzg32lo.js` | 144 191 | `/headset/audio_routine`: routine page (legacy `routineLegacy` / new `routineCommonParams`), volume/mic cards, effect cards, EQ presets editor, sound-mode routine |
| `index-WHcIArU5.js` | 64 275 | `/headset/audio_eq`: EQ page (ECharts editor, preset library, cloud preview dialog, import/export) |
| `index-BkAOL4Ty.js` | 12 740 | `/headset/audio_speaker`: 4 effect cards |
| `index-C8YYpMPR.js` | 7 703 | `/headset/audio_mic`: 4 mic cards |
| `index-DlclDdEp.js` | 7 453 | `/headset/audio_surround_thx`: THX spatial modes + 7.1 test video player |
| `audioMode-BToeZekz.js` | 1 789 | `/headset/audio_mode`: device sound-mode cards |
| `index-2a2mqfA3.js` | 14 482 | `/headset/audio_surround`: Xear 7.1 (speaker test, room size) |
| `eq-DcwrcT_4.js` | 10 813 | **HidEQ protocol**: PEQ get/set, prompt volume, auto-shutdown, factory reset, mode names |
| `crc32-BlQnRCeo.js` | 7 597 | **HID frame codec** (build/validate/parse; XOR checksum, despite the name) + notify dispatch |
| `webHIDBase-DFlt-On6.js` | 2 238 | `navigator.hid` lookup/open, `sendReport`, `inputreport` await helper |
| `eqSyncManager-YQQCV043.js` | 6 712 | EQ state sync |
| `hidEqChartOptions-CypytaiQ.js` | 107 273 | ECharts option builder for the PEQ curve editor |
| `getTargetHook-n3rBe2y4.js` | 6 561 | per-device HID report hooks (report ids 85/188/170) and mode maps |
| `index-Btnf0C9W.js` | 40 966 | `/headset/otherSetting`: firmware/OTA, power management, factory reset |
| `useOTAInfo-C7t-hS8F.js`, `ota-D5vpWLor.js` | 9 089 / 8 306 | OTA UI/plumbing |
| `index-h8iUWxYm.js` | 131 010 | theme drawer, wallpaper/lyrics/custom-image panels, K20 GT screen preview |
| `K20GtMidStack-*`, `K20-*`, `K20GT-*`, `G20-*`, `V9Turbo-*`, `speakerTopContainer-*`, `useHeadsetLightHID-*`, `useK20*HID-*`, `dotMatrixGifEncoder.worker-*` | | keyboard/speaker-specific pages and lighting HID hooks |

### 2.2 Chromium cache — proof the live build is what the installed app runs `[VERIFIED]`

`%APPDATA%\MCHOSEHUB\Cache\Cache_Data` uses the **blockfile** backend (`index`, `data_0..3`, magic
`0xC103CAC3`/`0xC104CAC3`) plus compressed entry files `f_XXXXXX` (gzip or brotli bodies).
Extraction: `renderer/scripts/cache-extract.js` → `renderer/cache-bodies/` (`_bodies.json`,
`_blockfile-urls.json`).

- 82 distinct `mchose`/`maicong` URLs recovered from `data_1`, including
  `https://cdn.mchose.com.cn/customPage/prod/index.html?v=1.1.26` and its assets — the Electron
  window loads the `customPage/prod` build, and also loads the www web app inside an iframe
  (`…/www.mchose.com.cn/?__iframe_load_key=…`).
- `f_000002` decompresses to 1 146 875 B and its **SHA-256 equals live
  `customPage/prod/assets/main-xA7mzCYt.js`** → the installed app runs exactly the documented CDN
  build (`renderer/scripts/map-cache.js`; 11/40 entries matched, the rest are the www app, fonts,
  keymaps, images).

Other URLs seen in the cache (useful project-wide):

```
https://cdn.mchose.com.cn/customPage/prod/assets/main-xA7mzCYt.js
https://cdn.mchose.com.cn/customPage/prod/assets/main-Bigq7yvw.css
https://cdn.mchose.com.cn/configCenter/config/global.json
https://cdn.mchose.com.cn/configCenter/custom/cardList.json?hash=0984e23f7f
https://cdn.mchose.com.cn/configCenter/custom/keyboardConfig.json?hash=1c85622bab
https://cdn.mchose.com.cn/configCenter/custom/keyboardPreset.json?hash=9c6d7b83ef
https://cdn.mchose.com.cn/configCenter/custom/mouseConfig.json?hash=89b67ad5c0
https://cdn.mchose.com.cn/configCenter/custom/newMouseConfig.json?hash=6817376c69
https://cdn.mchose.com.cn/configCenter/static/files/echarts.min_313ee9e3ab27.js
https://cdn.mchose.com.cn/OTA/PC/prod/otaFile/otaConfigApp_prod.json?hash=1e92eb2c71
https://cdn.mchose.com.cn/configCenter/static/documents/V9Pro2_CN_*.pdf
https://www.mchose.com.cn/tray.html
https://chat.maicong.cn/register/client/node
```

### 2.3 Local (installed app) artifacts

| Path | Content used here |
|---|---|
| `asar/out/preload/index.js` | **plaintext** `contextBridge.exposeInMainWorld("electronAPI", …)` — full IPC surface (§8) |
| `asar/bundled/renderer/refresh.html` + `refresh-assets/*` | local fallback/refresh page (Vue 3.5.35) — not the main UI (§7.10) |
| `asar/out/renderer/tips.html` + `tips-assets/tips-8dRZxa9O.js` | local tips/help overlay (Vue + Element Plus) |
| `%APPDATA%\MCHOSEHUB\files\config\mc_main_store_key.json` | persisted store (`webIndexList` device objects, `net-config-global`, `rendererVersion`, `user_config`) |
| `%APPDATA%\MCHOSEHUB\files\config\eq.json` | EQ preset library + current preset per device |
| `%APPDATA%\MCHOSEHUB\files\config\system.json` | app settings (theme, language, background, auto-start…) |
| `strings/*.txt` | pre-extracted strings of `.jsc` main bundle and local bundles |

### 2.4 Tooling written for this task (`renderer/scripts/`)

| script | purpose |
|---|---|
| `fetch-site.js`, `crawl.js`, `crawl2.js` | fetch/verify web app and renderer asset graphs (BFS, sha256 manifest) |
| `cache-scan.js`, `cache-extract.js`, `map-cache.js` | Chromium cache URL mining, body decompression, cache↔live identity |
| `extract-objects.js`, `extract-embedded-json.js`, `extract-enclosing-object.js`, `extract-audio-map.js` | recover data literals (i18n tables, device table, audio adapter table) |
| `i18n-report.js`, `prod-ns.js` | flatten/report locale tables |
| `api-inventory.js`, `api-bulk.js`, `list-api-calls.js`, `components.js` | `electronAPI` usage, per-chunk API method lists, Vue component inventory |
| `decode-configs2.js`, `fetch-configs.js`, `cardlist-summary.js` | decode remote `__compressBase64__` (base64→ZIP→data.json) config set |
| `cdp-shot.js`, `save-png.js` | headless Edge/CDP screenshot + DOM eval driver |
| `scan.js`, `find-routes.js`, `find-context.js`, `find-i18n.js`, `print-lines.js`, `dump-needles.js`, `json-summary.js` | ad-hoc static analysis |

Pretty-printed bundles: `renderer/pretty/` (web app), `renderer/pretty/prod/` (renderer).

> Tooling note: do **not** round-trip this Markdown (or any UTF-8 file) through Windows PowerShell
> 5.1 text cmdlets — `Get-Content`/`Set-Content` corrupt non-ASCII (it happened once and the doc was
> rewritten). Use Node or the editor tools.

---

## 3. Architecture as observed

```
Electron main (V8 bytecode)                        Renderer (remote SPA)
───────────────────────────                        ─────────────────────
window "M HUB"  ──── loads ────► cdn.mchose.com.cn/customPage/prod/index.html?v=<renderer version>
   │                                              │
   │  preload exposes window.electronAPI (415 ln)  │  Vue 3 + Pinia + vue-router (hash) + Element Plus
   │ ◄──────────── invoke/handle IPC ────────────► │  + iconfont + ECharts + Lottie; 2 local windows too
   │                                              │
   ├─ configCenter loader / global.json ◄─────────┤ fetch() remote config (versions, wallpapers, header)
   ├─ AudioSDK (C-Media) / AudioRuntimeTHX ◄──────┤ AudioSDKCall / AudioTHXCall (DSP + driver install)
   ├─ HID device permission (select-hid-device) ◄─┤ navigator.hid open/sendReport/inputreport
   ├─ store: files/config/*.json ◄────────────────┤ getStore/setStore/watchAllStore, getConfig/setConfig
   ├─ tray.html / tips.html / refresh.html (local)│
   └─ iframe host ◄───────────────────────────────┤ www.mchose.com.cn web driver embedded for KB/mouse
```

Notes:
- The renderer page is served from the CDN origin while the window is created by the main process;
  the asar also contains `mchose-renderer://` scheme strings and an env table (dev/test/prod hosts)
  in the main bundle strings — consistent with the main process loading or proxying the remote URL.
  `[INFERRED for scheme details — main-re/native-re own that]`
- **Hybrid UI**: only theme + headset are implemented natively. The `/index` "home" route is an
  **iframe host** that loads the browser web driver's connect page
  (`getEnvConfigs()[env].urlIndex` → `https://www.mchose.com.cn/#/connectDevice`) with a
  `?__iframe_load_key=<n>_<timestamp>` cache-buster, retries every 3 s up to 10 times
  (`retryLoadRenderer({forceReload:false})`), and talks to it through `window.iframeBridge`
  (`callOnce`: `track`, `k20GtShellRequestStateSync`, `onPCMainStoreChange`, `trayEnterItem`;
  `callSync`: `getCurrentRouteName`, `getWebCapabilities`, `loadGlobalConfig`, `uninstall`).
  Device discovery/connection UI therefore lives in the **web app**, not in the native renderer.
  `[VERIFIED from code + cache URL]`

---

## 4. Route map

### 4.1 Electron renderer (`customPage/prod`) — verified router `[VERIFIED]`

Only **four top-level routes**; audio device UI lives under `/headset`. Router source:
`main-xA7mzCYt.js` (`const routes = [...]`; `createRouter({history: createWebHashHistory(), routes})`).

| path | name | meta.title | component chunk |
|---|---|---|---|
| `/` | – | – | redirect → `/index` |
| `/index` | `Index` | 首页 | **iframe host**, bundled in `main-xA7mzCYt.js` — renders `getEnvConfigs()[env].urlIndex` (= `https://www.mchose.com.cn/#/connectDevice`) with `?__iframe_load_key=<n>_<ts>` |
| `/themeSetting` | `ThemeSetting` | 主题设置页 | `index-XWBRFluS.js` |
| `/headset` | `Headset` | 耳机页 | `headsetPage-DNy5Z21r.js` (sidebar layout) |

Children of `/headset`:

| child | name | chunk | purpose |
|---|---|---|---|
| `audio_routine` | `AudioRoutine` | `index-Cbzg32lo.js` | 常规参数/常用参数: volume, mic, effects, sound-mode routine, EQ presets editor |
| `audio_eq` | `AudioEq` | `index-WHcIArU5.js` | 均衡器: chart editor, preset library, cloud preview, import/export |
| `audio_mode` | `AudioMode` | `audioMode-BToeZekz.js` | 音效模式 (device mode cards) |
| `audio_surround` | `AudioSurround` | `index-2a2mqfA3.js` | Xear 虚拟7.1环绕 |
| `audio_surround_thx` | `AudioSurroundThx` | `index-DlclDdEp.js` | THX 空间音效 + 7.1 test video |
| `audio_speaker` | `AudioSpeaker` | `index-BkAOL4Ty.js` | 播放设置: 4 effect cards |
| `audio_mic` | `AudioMic` | `index-C8YYpMPR.js` | 麦克风设置: 4 mic cards |
| `lighting` | `Lighting` | `index-CDqDg8Br.js` | generic lighting |
| `lighting_g20` | `LightingG20` | `G20-zF6Ax0We.js` | G20 lighting |
| `lighting_k20` | `LightingK20` | `K20-CbRxmR7r.js` | K20 lighting |
| `lighting_k20_gt` | `LightingK20GT` | `K20GT-C6g8s9fJ.js` | K20 GT lighting (+screen) |
| `lighting_v9turbo` | `LightingV9Turbo` | `V9Turbo--0b51NXm.js` | V9 Turbo lighting |
| `screen_setting` | `ScreenSetting` | `index-h8iUWxYm.js` | 屏幕显示 (K20 GT only) |
| `otherSetting` | `otherSetting` | `index-Btnf0C9W.js` | 其他设置 (firmware/OTA/power/factory) |

Sidebar construction (`headsetPage`), rules observed:
- **new layout** items: `menus.CommonParams` → audio_routine (icon `icon-a-commonsettings_linear`),
  `menus.SpeakerSettings` → audio_speaker (`icon-a-soundmode_linear`),
  `menus.MicSettings` → audio_mic (`icon-a-micon_linear`),
  `menus.THXSurround` → audio_surround_thx (`icon-thx_linear`).
- **legacy** items: `menus.GeneralParameters` → audio_routine (`icon-a-speakeron_linear`, dataTrack
  `4002-…`), `menus.Equalizer` → audio_eq (`icon-setup2_linear`), `menus.VirtualSurround` →
  audio_surround (`icon-surround_linear`), plus `menus.SoundMode` → audio_mode inserted at index 2
  only when capability `u.value` is true.
- Then (both): `menus.LightingSettings` (only for S9 PRO, G9 PRO, S9 PRO PID14429, G9 PRO PID14429,
  G20 PRO, G20, K20, K20 PRO, K20 GT, G9 V2, X9 — plus V9 TURBO+ variants when a flag is set),
  `menus.ScreenSettings` (only `MCHOSE K20 GT`), and `menus.OtherSettings` last (red dot when
  `hasNewVerDongle || hasNewVerHeadset`).
- THX tab title is overridden with `thxSurround.pageTitle` ("空间音效").
- Device route for lighting is chosen by product key → `lighting_g20|lighting_k20|lighting_k20_gt|
  lighting_v9turbo|lighting`.

### 4.2 Browser web driver (`www.mchose.com.cn`) `[VERIFIED]`

Hash routes in the web app bundle (`purify.es-CVNN4l_y.js`):

```
/            /index       /home        /connectDevice   /detail
/newMouse    /keyboard    /setting     /themeSetting    /autoBoardSetting
/lightSync   /expertView  /headerView  /test-keyboard   /test-mouse
/advertisement           /feedback    /headset/audio_routine
```

The web driver explicitly excludes audio devices: `home.currentSupportDevices` says audio devices
must use the downloaded client ("音频设备请访问官网 … 点击【驱动】—【下载 M HUB 客户端】体验").
Headset audio UI therefore exists only in the client renderer.

### 4.3 Local auxiliary pages

- `bundled/renderer/refresh.html` — fallback when the remote renderer fails (§7.10).
- `out/renderer/tips.html` — help/tips overlay `[VERIFIED exists; content not analysed]`.
- `https://www.mchose.com.cn/tray.html` — custom tray window (created by main via
  `createTrayWindow`/`customTray`).

---

## 5. Shell layout (header / navigation)

From `main-xA7mzCYt.js` (`App`, `commonHeader`) and the web app's `connectHeader`:

- **Custom window frame**: minimise / maximise-restore / close via
  `electronAPI.minimizeWindow|toggleMax|closeWindow|hideWindow|isMaximized|onMaximize|onUnmaximize`;
  state persistence `getWindowStateSnapshot`, `getWindowSize/setWindowSize`, `setBounds`,
  `setWindowMInSize`, `setZoomFactor`.
- **`commonHeader`** (i18n `commonHeader.*`): official store, "My device", back / back-home, theme
  popover (`popoverTheme`), settings (`popoverSetting`), related app/game auto-switch
  (`popoverRelatedApp`), min/max/close, feedback (`feedback`), reward survey (`prizeQuiz`),
  performance-mode texts (`performanceOnDesc`/`performanceOffDesc`), offline notice (`offline`).
- Header buttons are remote-driven: `customFields.headerBtnConfigs` (entries with `type:"out"`,
  `routeName`, per-language `image`, `hide`; currently one DingTalk survey link).
- `AudioSDKStatePanel` surfaces audio-runtime install/ready state.
- Theme: `themeSetting.*` — light `霜茶白` / dark `玄潭黑` / follow system; background (`bg`), custom
  background (home only), upload image, card blur (`blurCard`), background blur (`blurBg`).
  Wallpaper catalogue is remote (`bgConfigV2`: official / 七夕 / 凡人 collab / team categories;
  static jpg or dynamic webm, with schedules).
- Web-app chrome rule (browser): routes `["/connectDevice","/setting","/feedback","/expertView",
  "/autoBoardSetting","/themeSetting","/lightSync"]` hide the normal header.
- Router `afterEach` toggles background mode: `Index` route → `setBgImgConfig({isIndex:true})`,
  otherwise `isIndex:false` and closes the drawer (`Qd(false)`).

---

## 6. Headset device page (`headsetPage`) structure

Components: `headsetPage`, `HeadsetSidebarHeader`, `left`/`leftV2`, `right`/`rightV2`, `main`,
`ThxInstallDialog`, `instructions` (pdf.js manual), `VuePdfEmbed`, `loadingIcon`, `index`.

Observed behaviour:

- **Sidebar header**: render name (`deviceConfig.renderName`), connection-mode icon
  (`icon-usb_linear` when `connectMode===0`, else `icon-wifi_linear`), battery + charge status
  (battery shown only when numeric). `devicePage.wiredMode` vs `"2.4G"` label.
- **Battery** `[VERIFIED]`:
  - new layout: HidEQ `GET_BATTERY_SLEEP` (cmd 11), then listens for `inputreport` with the battery
    report id and decodes the `BatterySleep` notify (`{asleep,charging,percent}`); logs
    "获取到电量信息（HidEQ 0x0B）".
  - legacy: device lookup by `{productId, vendorId, productName, reportId:85}` + legacy getter.
  - `isSleep` → `batteryLevel = null` + sleep mask (`devicePage.headphoneSleepStatus` / `…Tips`).
- **Disconnect mask**: `devicePage.deviceLostLink` with type hints (`keyboardSleepStatus`,
  `mouseSleepStatus`, `needLinkWireless`, `…HeadphoneTips`, `notAdminRunningHeadset`); returns to
  `/index` when the current route is not `Index`.
- **Audio-runtime guard**: initialises AudioSDK/THX on entry; failure → THX install dialog
  (`devicePage.thxInstallDialogTitle/…Desc1/…Desc2`, recovery texts) and
  `AudioTHXDownloadInstaller` / `AudioTHXInstallInstaller` / `AudioSDKInstallDriver` flows.
- **Firmware/OTA**: versions via HidEQ OTA `0x80` (new layout) or `getFirmwareVer` (legacy);
  red dot on Other settings when newer; download through `download`/`otaDevice`/`updater.*`;
  restart prompt (`devicePage.restartNotice/restartNow`).
- **User manual**: `instructions` opens bundled pdf.js with
  `deviceConfig.pdfSourceByLang[lang]` (e.g. V9 Pro 2:
  `…/configCenter/static/documents/V9Pro2_CN_xuhmqow6i2ramwo7.pdf`) or legacy `pdfSource`
  (`V9.pdf`, `V9t.pdf`, `V9tBase.pdf`, `K20Pro.pdf`, `K20GT.pdf`, `G20.pdf`, `K20.pdf`, `X9.pdf`).
- **BetterNCM** (NetEase lyric plugin): `installBetterNcm`/`uninstallBetterNcm`, progress event
  `betterNcmInstallProgress`, `otherSettings.betterOtaTitle`.

---

## 7. Feature specifications (headset)

Legend: **Where** = screen; **API** = renderer-visible call. UI code uses a device proxy
`p.get(method, args, default)` / `p.set(method, args)`; dispatch per runtime — see §8.1.

### 7.1 Audio routine — legacy 常规参数 / new 常用参数 (`/headset/audio_routine`)

Components: `routineCommonParams`, `routineLegacy`, `volume`, `volumeBroad`, `voiceBroadcast`, `mic`,
`micListen`, `micNoiseReduction`, `micAINoiseReduction`, `soundModeRoutinePanel`,
`eqRoutineEditorPanel`, `eqRoutineEditorOverlay`, `eqRoutineNameDialog`, `EqPresetPreviewDialog`,
`eqAuditionPanel`, effect cards (`surroundAmplification`, `brightened`, `lowFrequency`,
`smartVolume`, `vocalsClarity`).

**Page composition `[VERIFIED]`** — route component switches on `deviceConfig.isNewLayout`:

- **legacy → `routineLegacy` (V9 PRO landing screen)**, single column:
  1. hero: large product image (`productImgSrc`; 3D preview for K20 GT), entrance animation;
  2. footer with **3 columns**:
     - col 1: `volume` card · (`volumeBroad` only when `ifVolumeBroadcast`) · `mic` card;
     - col 2: `surroundAmplification` (环绕扩增) · `lowFrequency` (动态低频);
     - col 3: `smartVolume` (智能音量) · `vocalsClarity` (人声清晰化).
  A `page-v9-turbo` class variant adjusts layout/extras for V9 Turbo.
- **new layout → `routineCommonParams`**: "routine-common" 2-column layout — left
  `routine-common__left-main` (device image + volume/mic), right `routine-common__card--right`,
  injecting HidEQ routine state (`hidEqRoutine`) and using `ifVolumeBroadcast`, `isSpeaker`,
  `productImgSrc`, `volumeRef`, `micRef`.

| Feature | Label (zh / en) | Control | API / range |
|---|---|---|---|
| Speaker volume | `routine.Volume` 音量 / Volume | slider + mute | `getVolumeScalarControl(channel,"speaker")`, `setVolumeScalarControl(channel,val,"speaker")`, `getMuteControl("speaker")`, `setMuteControl(bool,"speaker")` |
| Voice prompt volume | `routine.beepTitle` 语音提示 / Sound Notification; `routine.beepTitleDesc` | switch + slider | `getPromptVolume`/`setPromptVolume`; HidEQ cmds 13/33; value **1..16** — **new-layout (HidEQ) path only**; not present in `routineLegacy` (legacy devices `[UNKNOWN]`) |
| Voice broadcast | `routine.Speech` (+ `…Desc`, long-press device keys) | switch | notify `VoiceBroadcast` (HID cmd 2) |
| AI noise reduction | `routine.MicAI` 开启AI降噪 | switch | `getAINREnable`/`setAINREnable` (+ `getAINRLevel`/`setAINRLevel`) |
| Steady-state noise suppression | `routine.MicNoiseReduction` 稳态噪声抑制 | switch (+level on some) | `getNR_Enable`/`setNR_Enable` |
| Magic voice | `routine.MoYin` 魔幻音效：`MoYin_0..3` = 怪兽 / 卡通 / 男声 / 女声 | switch + 4 options | `getEnable_MAGICVOICE`/`setEnable_MAGICVOICE`, `getMagicVoice_Selection`/`setMagicVoice_Selection(type)` (THX: `MONSTER_V1`…`FEMALE_VOICE_V1`) |
| Mic listen | `routine.MicListen` 麦克风监听 | switch | `getMicListenStatus`/`setMicListenStatus` + `getDeviceFriendlyName("mic")` |
| Surround amplification | `routine.SurroundAmplification` 环绕扩增 | card | `getEnable_Channel_COPY_LFX`/`setEnable_Channel_COPY_LFX` |
| Audio brightening | `routine.AudioBright` 音频明亮化 | card | `getEnable_AUDIOBRILLIANT_LFX`, `get/setAUDIOBRILLIANT_LEVEL` |
| Dynamic low frequency | `routine.DynamicLF` 动态低频 | card | `getEnable_VIRTUALBASS_LFX`, `get/setVIRTUALBASS_Level`, `…CutOffFrequency` |
| Smart volume | `routine.SmartVolume` 智能音量 | card | `getEnable_ADAPTIVEVOLUME_GFX`, `get/setADAPTIVEVOLUME_LEVEL`, `…MODE` |
| Vocal clarity | `routine.VocalClarity` 人声清晰化 | card | `getEnable_VOICECLARITY_LFX`, `get/setVOICECLARITY_LEVEL`, `…NOISESUPP_LEVEL` |
| EQ presets / sound modes | `routine.soundMode*`, `eq.*` | see §7.4 | — |
| Import/export | `eq.import` 本地导入 / `eq.export` | button | `importData()`, `exportData(data, defaultPath)` |
| Windows sound panel | `routine.WindowsAudioDevices` | button | `openSoundSetting()` |
| Volume watch | — | background | `startWatchAudioVolume`/`stopWatchAudioVolume`, event `onAudioVolumeChange` |

### 7.2 Mic page (`/headset/audio_mic`, new layout)

| Card | Label | API |
|---|---|---|
| Mic leveling | `micSettings.micLeveling` 麦克风调平 | `setEnable_MICVOLUMELEVELING`/`getEnable_MICVOLUMELEVELING`, `set/getMICVOLUMELEVELING_LEVEL` — slider 0…100 step 1 (THX marks 0 `naturalDynamics` → 100 `stableVolume`, reset 0.5) |
| Vocal clarity | `micSettings.vocalClarity` 人声清晰化 | `set/getMicVoiceClarityEnabled`, `set/getMicVoiceClarityLevel` |
| Mic noise reduction | `micSettings.mciNoise` 麦克风降噪 | `set/getAINREnable` (+ `getDeviceFriendlyName("mic")`) |
| Mic monitoring | `micSettings.micListen` 麦克风侦听 | `set/getMicListenStatus` |

Mic-disabled recovery (`devicePage.micDisabled`, `…micDisabledTitle`, `step/step1/step2/step3`):
open Windows sound panel → enable recording device → refresh. Speaker variant:
`devicePage.speakerDisabled` + same steps.

### 7.3 Speaker settings (`/headset/audio_speaker`) — 4 effect cards `[VERIFIED in detail]`

Page = `adv-page` + 2×2 `adv-grid`, cards in order **treble / bass / volume balance / vocal
clarity**, wrapped in `animeCeilTransition` ("up"). Every switch/slider is debounced 150 ms (max
300 ms); rows disable while the switch is off. Per-card reset quirks: the **bass** and
**volume-balance** reset buttons are rendered only on THX devices; the **treble** reset only
rewrites the level (never disables the switch). Level rows show a bare number — only the bass
cutoff row carries a unit (`unitHz`) and only the vocal-clarity noise row carries `unitDb`.
C-Media note: the `20..20000 Hz` cutoff and `0..1` noise ranges exist only on the THX branches,
where those rows are not rendered, so the bold ranges below are the effective cmedia ones.

Runtime differences by `deviceConfig.audioRuntimeType` (`cmedia` vs `thx`):

| Card | i18n | Switch API | Value API | cmedia range | thx range | THX end labels | Defaults (cmedia / thx) |
|---|---|---|---|---|---|---|---|
| Treble | `speakerSettings.trebleEnhancement` 高频增强 | `setEnable_AUDIOBRILLIANT_LFX` | `setAUDIOBRILLIANT_LEVEL` | 0..10 step 1 | 0..100 step 10 | 0 `softNatural` 柔和自然, 100 `clearCrisp` 通透清脆 | 3 / 0 |
| Bass | `speakerSettings.bassEnhancement` 低音增强 (+`routine.Freqtrap` 频段截取) | `setEnable_VIRTUALBASS_LFX` | `setVIRTUALBASS_Level`, `setVIRTUALBASS_CutOffFrequency` | **level 0..10 step 1**; **cutoff 100..400 Hz step 50** (marks 100/200/300/400) | level 10..100 step 1 (cutoff row hidden) | 10 `softNatural`, 100 `powerfulBass` 强劲低音 | level 5 / 0.55 ; cutoff 200 Hz |
| Volume balance | `speakerSettings.volumeBalance` 音量均衡 | `setEnable_ADAPTIVEVOLUME_GFX` | `setADAPTIVEVOLUME_LEVEL`, `setADAPTIVEVOLUME_MODE` (radio 0 `routine.MusicMode` 音乐模式(高动态) / 1 `routine.VoiceMode` 语音模式(低动态), cmedia only) | **level 0..100 step 1** | level 0..100 step 1 | 0 `naturalDynamics`, 100 `stableVolume` | 0 (factory reset 50) / 0 |
| Vocal clarity | `speakerSettings.vocalEnhancement` 人声增强 (+`routine.NCut` 噪音抑制) | `setEnable_VOICECLARITY_LFX` | `setVOICECLARITY_LEVEL`, `setVOICECLARITY_NOISESUPP_LEVEL` | level 0..10 step 1; **noise 0..20 step 5** (marks 0/5/10/15/20, dB) | level 0..100 step 1 (noise row hidden) | 0 `keepBackground`, 100 `enhanceVoice` | 3 / 0 |

Labels used: 强度 (`routine.Intensity`), 低音强度 (`routine.DYIntensity`), 清晰强度
(`routine.QXIntensity`), units `speakerSettings.unitDb` / `unitHz`.

### 7.4 Equalizer — two systems

**(a) Legacy / C-Media 10-band EQ (the connected V9 PRO path)** `[VERIFIED: eq.json + preload]`

- Storage `files/config/eq.json`:
  `{"game":[{isSystem,name,data[10],group?}],"music":[…],"current":{"MCHOSE V9 PRO PID14429":"默认"}}`.
- Preset values are integers in dB; observed **-13 … +14** (UI slider range UNKNOWN, likely ±12).
- APIs: `GetEnable_EQ_GFX()`, `setEnable_EQ_GFX(bool)`, `getEQ_Slider(index)`,
  `setEQ_Slider(index,value)` — index 0..9 (band 1..10). THX maps these to
  `eq_enabled` + `eq_band_{n}_gain_db` (§7.6b).
- Factory presets (zh): game — `默认`, `无畏契约1/2/3` (group 无畏契约), `CS1`, `CS2`, `Apex1`,
  `Apex2`, `绝地求生1/2/3`, `CF1`, `三角洲1`; music — `舞曲, 饶舌曲, 重金属, 爵士, 抒情摇滚, 摇滚,
  现场, 高音, 低音, 古典乐, 声乐`.
- UI: ECharts curve editor (x = 10 bands; axis captions `eq.bandBass`/`bandMid`/`bandHigh`),
  preset library panel (`routine.eqPresetLibrary`), tabs (`routine.eqMyTab` "我的"), hover-apply
  (`routine.eqItemHoverApply`), audition (`routine.eqAudition*`, `eqAuditionPipWindow` helper
  window), rename/save-as/delete/import/export, cloud community share (review states).

**(b) New-layout on-device PEQ (HidEQ)** `[VERIFIED in detail — eq-DcwrcT_4.js]`

- 3 modes: `GAME_1=0, GAME_2=1, GAME_3=2` (labels `eq.game1..3` "EQ 模式 1/2/3").
- Parametric EQ: per point `{fc u16 Hz, gain i16 (/10 dB, clamp -12.0…+12.0), q u8 (/10),
  type u8, enable u8===1}` — 7 bytes per point; plus `totalGain`.
- Commands (HID frame `cmd`, §10): `1 SET_EQ_MODE`, `4 GET_EQ_STATE [mode,type]`,
  `5 GET_EQ_MODE_DESC [mode]` (returns mode name, UTF-8, zero-terminated, ≤54 B on set),
  `11 GET_BATTERY_SLEEP`, `12 GET_CURRENT_EQ_MODE`, `13/33 GET/SET_PROMPT_VOLUME`,
  `14/26 GET/SET_AUTO_SHUTDOWN`, `16 SET_PEQ_ALL`, `18 SET_PEQ_ALL_GAIN`,
  `24 SET_EQ_MODE_NAME`, `25 SET_EQ_MODE_RESTORE_DEFAULT`, `27 FACTORY_RESET`.
- Field selectors for `GET_EQ_STATE`: `255 ALL_POINTS, 254 ALL_FC, 253 ALL_GAIN, 252 ALL_Q,
  251 ALL_TYPE, 250 ALL_ENABLE, 249 TOTAL_GAIN`.
- Multi-packet replies: byte[3] = `totalPackets<<4 | currentPacket`; point records start at offset 6
  (mode, totalGain u16 at 2, pointCount at 4, packet info at 5).
- `SET_PEQ_ALL_GAIN` payload: `[mode, count, gainHi, gainLo, …]` (uint16 = dB×10, big-endian).
- `SET_PEQ_ALL` (cmd 16) is **declared but never used** by this UI build — writes go through
  `SET_PEQ_ALL_GAIN` (18) and the single-point / mode commands; only the read side uses cmd 4 with
  the field selectors above. `[VERIFIED negative finding]`
- Sound-mode routine UI: `soundModeRoutinePanel` ("声音模式", `routine.soundModeTitle`,
  `soundModeBadge` "模式{n}", active/off, rename, edit EQ, auto-save hints).

### 7.5 Virtual 7.1 surround — legacy (`/headset/audio_surround`, Xear) `[VERIFIED]`

i18n `surround`: switch `Xear虚拟7.1环绕`; mode `音乐/游戏模式` vs `电影模式`; speaker test
`start`/`stop`; room size `small/mid/big`; help popover warning 7.1 is not for FPS.
APIs: `getXearSurroundEnable`/`setXearSurroundEnable`, `getXearSurroundMode`/`setXearSurroundMode`,
`getXearSurroundRoom`/`setXearSurroundRoom`, `playTestSound(value)`/`stopTestSound()`.

### 7.6 THX spatial audio — new layout (`/headset/audio_surround_thx`) `[VERIFIED]`

Three exclusive mode cards (`btns-row`, each title+desc+check icon):

| Index | UI state | Calls |
|---|---|---|
| 0 | `thxSurround.spatialAudioOff` 关闭空间音效 + `pureStereoDesc` | `setXearSurroundEnable(false)`, `setUpMixEnabled(false)` |
| 1 | `menus.THXSurround` (THX空间音效) + `virtual71Desc` | `setXearSurroundEnable(true)`, `setUpMixEnabled(false)` |
| 2 | `thxSurround.thxReverbSpatialAudio` THX混响空间音效 + `reverbDesc` | `setXearSurroundEnable(true)`, `setUpMixEnabled(true)` |

Read on mount: `getXearSurroundEnable`, `getUpMixEnabled`. Below: `playVideoDesc` + `thxVideoPlay`:
custom muted `<video>` with fixed clip
`https://cdn.mchose.com.cn/configCenter/static/videos/7.1_q8v7fiodm43buqxg.mp4`,
`AudioTHXGet7TestVideoInfo()` → `{endpointId, layout, videoPath}` (defaults `headphones`/`7.1`) and
`AudioTHXAudioPlayerControl({productKey, action:"play"|"pause"|"resume"|"stop"|"seek",
payload:{endpointId, layout, positionSeconds?, …hwid}})`. Player UI: centre play/pause, control
panel, timeline seek; the `<video>` is muted (sound comes from the THX runtime).

### 7.7 Other settings (`/headset/otherSetting`) `[VERIFIED strings + APIs]`

| Group | Items |
|---|---|
| Firmware | current/new version, "已最新"/`goUpdate`, device/dongle/headset/base version rows (`headsetV`, `dongleV`, `baseV`, `usbV`), `releaseNotes`, `upgradeNow`/`noUpgrade`, progress (`downloading`/`updating`/`updatingNote`), success/failure texts (`otaSuc`, `otaSucDongle`, `otaSucHeadset`, `otaSucBoth`, `otaFail`, `otaFailResult`, `deviceReconnectFail`), manual download tips, `downloadNewClient` fallback |
| Power management | `otherSettings.powerManagement` 电源管理, `autoShutdown` 耳机智能关机 with minute presets + `neverSleep` 永不 (new layout uses `newlayoutAutoShutdownDesc`); HID `GET/SET_AUTO_SHUTDOWN` with uint16 minutes (0 = never) |
| Device power | `speakerShutdown` 设备关机 (+confirm), `speakerShutdownSuccess/Failed`, legacy `speakerShutdownDesc` ("驱动仅能关机，不能开机") |
| Factory reset | `restoreFactory` 恢复出厂设置 + `confirmFactoryReset`; HID `FACTORY_RESET` target `HEADSET=1 / DONGLE=2 / ALL=3` |
| Misc | `inDevelopment` 功能开发中，敬请期待..., BetterNCM install (§6) |

OTA plumbing: `useOTAInfo` hook (`OTAInfo`), `ota` module, `electronAPI.otaDevice(p)`,
`getFirmwareBinData(p)`, `getUsbVersion(p)`, `getFWVersionFromUsbDescriptor(p)`,
`getUsbDeviceDescriptor(p)`, `getUsbStringDescriptor(p)`, event `onOtaMessage`; app updater:
`updater.getState/startSilentDownload/startManualUpdate/applyDownloadedUpdateNow` +
`updater:stateChanged`, remote `otaConfigApp_prod.json`.

### 7.8 Lighting (device-family, same shell)

`menus.LightingSettings` → per-device route; i18n `light.*`: effects `常亮/呼吸/繁星流彩/随波逐流/
流光慢/流光快/音乐律动`, direction, speed, brightness, colour picker (`colorjoe`), RGB sync
(`神光同步`), K20 modes (`k20Mode1..7`), smart light (30-min low brightness). HID hooks:
`useHeadsetLightHID`, `useK20HID`, `useK20TurboHID`, `useG20HID`; notify cmds 3/5/6/10/14 (§10).

### 7.9 Screen settings (K20 GT only)

`/headset/screen_setting`; i18n `screenSettings` (~300 keys): on/off, colour, wallpaper presets
(digital clock, music spectrum, campus/workplace/pets/games/cyber/memes), custom image upload
(PNG/JPG/GIF, GIF first 30 frames), AI image generation (20/day), text themes (alignment, font size,
scroll effects), lyrics display (BETA, 10 animation types), batch delete, upload progress,
"sync colour to lighting". Not present for V9 PRO.

### 7.10 Local fallback / error page (`asar/bundled/renderer/refresh.html`) `[VERIFIED]`

Bundled Vue 3.5.35 mini-app shown when the remote renderer fails to load:

- HTML: `#app`; loads `refresh-assets/refresh-*.js|css` + `publicResources/lottie.min.js`; calls
  `window.electronAPI?.reportRendererReady?.()`.
- Copy keys: `index.loadFail` ("加载失败"), `index.loadFailDesc` ("请点击刷新按钮以重新加载，或检查
  网络连接、重启驱动"), retry `common.refresh`, window buttons `commonHeader.popoverMin/popoverUnmax`.
- Same shell plumbing: store API, config API (`getConfig`/`setConfig`, `getDefaultLang`,
  `getThemeValue`), identity (`getRendererBuildInfo`, `getCurEnv`), window controls, `uninstallDriver`.
- Retry: `retryLoadRenderer({forceReload:false})`, `probeRendererReachable`; `vc-redist:check-x64`
  is exposed for the "missing MSVC runtime" dialog (`musicDance.needWinVc*` texts).

`tips.html` + `tips-assets/tips-8dRZxa9O.js` (1.7 MB) is a separate help/tips overlay `[VERIFIED
exists; contents not analysed]`.

### 7.11 Theme / wallpaper / settings / account / feedback / tray / home `[VERIFIED strings/APIs]`

- **Home** `/index` `[VERIFIED]`: **not a native page** — the component is an iframe host that
  embeds `curEnvConfig.urlIndex` (`https://www.mchose.com.cn/#/connectDevice`) with
  `__iframe_load_key`, 3 s × 10 auto-retry, `backgroundLoading` flag, `iframeBridge` telemetry
  (`track` page/event ids), K20 GT state sync, `onPCMainStoreChange`, `trayEnterItem`, and a manual
  reload path; it restores `pendingRefreshIframeUrl` from `sessionStorage` on mount. So the device
  list/connect/first-guide UI is the web driver's (`connectDevice.*` i18n in `renderer/i18n/`), and
  the client shell only provides window chrome, tray, updater and the headset/theme native pages.
- Theme drawer (`index-h8iUWxYm`): `drawer`, `themeStatusDrawer`, `CustomImageUploadDialog`,
  `CustomImageAiDrawer`, `CustomImagePreset`, `QixiPreset`, `preset`, `custom`,
  `LyricsSettingsPanel`, `LyricsAnimationPreview`, `K20GtScreenPreview`.
- `setting.*`: version, startup (auto-start, start minimised to tray), language, close behaviour
  (exit vs minimise), copyright.
- Account: `userProfile.*` (shares/downloads/favourites/likes; WeChat/phone/email binding; M1–M4
  levels), `wechatLogin.*` (30-day login, privacy policy + user agreement full texts, email login,
  bind email, test-env login), `common.clickLogin/logout`. APIs: `getUserConfig`/`setUserConfig`,
  legacy-localStorage/IndexedDB access, `openBrowser(url)`.
- Feedback: `satisfaction.*`, `https://www.maicong.cn/mchose/questionnaire`,
  `https://alidocs.dingtalk.com/...` (header button), `FeedbackPage` chunk in the web app.
- Tray: `tray.open` 打开M HUB / `tray.quit` 退出; window via `createTrayWindow`/`customTray`.
- Media/lyrics: `startLyricsMonitor`/`stopLyricsMonitor`/`isLyricsMonitorRunning`,
  `readQQMusicQrc`, `resolveQQMusicLyricPaths`, `readKuGouKrc`, `resolveKuGouLyricPath`,
  `getKuGouPlaybackState`, `getNcmInstallPath`; event `onLyricsMediaUpdate`; SMTC worker.
- App/game list: `scanAppList()`, `scanGameList()`, `getExeDetailInfo(p)`,
  `startListenActiveAppChange(p)`/`stopListenActiveAppChange()`, event `onActiveAppChange`
  ("关联游戏/应用，自动切换板载").

### 7.12 THX property contract + AudioSDK→THX adapter `[VERIFIED from code]`

One UI drives either runtime: panels call legacy C-Media names (`p.set("setXearSurroundRoom", …)`);
when `audioRuntimeType === "thx"`, the adapter translates them into THX
`AudioTHXCall({method, args})` calls. Full table: **56 cases** in
`renderer/notes/audio-method-map.json` (extracted by `scripts/extract-audio-map.js`).

**Endpoints** (`endpoint_id`): `""` global, `"headphones"`, `"microphone"`.
**Endpoint params** (`get/setEndpointParameter`, `parameter_id`): `100` FRIENDLY_NAME,
`101` FIRMWARE_VERSION, `200` VOLUME_SCALAR, `201` MUTE_CONTROL, `300` SET_TO_DEFAULT.
**Preset groups**: `device_presets` (headphones; `high_dyn`/`low_dyn`), `room_presets`
(`large`/`medium`/`small`), `microphone_presets`, `microphone_voice_morphing_presets`.
**Property types**: `PropertyType_Bool | PropertyType_Float | PropertyType_String`.

| Legacy method (UI) | THX method | endpoint | property / param | type |
|---|---|---|---|---|
| `getDeviceFriendlyName(type)` | getEndpointParameter | `mic`→microphone else headphones | 100 | – |
| `getFirmwareVer` | getEndpointParameter | headphones | 101 | – |
| `get/setVolumeScalarControl(t,type)` | get/setEndpointParameter | by type | 200 | Float |
| `get/setMuteControl(t)` | get/setEndpointParameter | by type | 201 | Bool |
| `setToDefault` | setEndpointParameter | headphones | 300 | – |
| `get/setAINREnable` | get/setProperty | microphone | `mic_noise_cancellation_enabled` | Bool |
| `get/setAINRLevel` | get/setProperty | microphone | `mic_noise_cancellation_level` | Float |
| `get/setEnable_MAGICVOICE` | get/setProperty | microphone | `mic_voice_morphing_enabled` | Bool |
| `get/setMagicVoice_Selection` | get/setProperty | microphone | `mic_voice_morphing_mode` | String |
| `get/setEnable_MICVOLUMELEVELING` | get/setProperty | microphone | `mic_volume_leveling_enabled` | Bool |
| `get/setMICVOLUMELEVELING_LEVEL` | get/setProperty | microphone | `mic_volume_leveling_level` | Float |
| `get/setMicVoiceClarityEnabled` | get/setProperty | microphone | `mic_voice_clarity_enabled` | Bool |
| `get/setMicVoiceClarityLevel` | get/setProperty | microphone | `mic_voice_clarity_level` | Float |
| `GetEnable_EQ_GFX`/`setEnable_EQ_GFX` | get/setProperty | headphones | `eq_enabled` | Bool |
| `get/setEQ_Slider(i)` | get/setProperty | headphones | `eq_band_{i+1}_gain_db` (i=0..9) | Float |
| `get/setXearSurroundEnable` | get/setProperty | headphones | `spatial_enabled` | Bool |
| `get/setUpMixEnabled` | get/setProperty | headphones | `upmix_enabled` | Bool |
| `get/setXearSurroundRoom(i)` | getSelectedPreset/selectPreset | headphones | `room_presets` = large/medium/small | – |
| `get/setEnable_AUDIOBRILLIANT_LFX` | get/setProperty | headphones | `audio_brightening_enabled` | Bool |
| `get/setAUDIOBRILLIANT_LEVEL` | get/setProperty | headphones | `audio_brightening_level` | Float |
| `get/setEnable_VIRTUALBASS_LFX` | get/setProperty | headphones | `bassboost_enabled` | Bool |
| `get/setVIRTUALBASS_Level` | get/setProperty | headphones | `bassboost_level` | Float |
| `get/setVIRTUALBASS_CutOffFrequency` | get/setProperty | headphones | `bassboost_freq` | Float |
| `get/setEnable_ADAPTIVEVOLUME_GFX` | get/setProperty | headphones | `volume_leveling_enabled` | Bool |
| `get/setADAPTIVEVOLUME_LEVEL` | get/setProperty | headphones | `volume_leveling_level` | Float |
| `get/setADAPTIVEVOLUME_MODE` | getSelectedPreset/selectPreset | headphones | `device_presets` = high_dyn/low_dyn | – |
| `get/setEnable_VOICECLARITY_LFX` | get/setProperty | headphones | `voice_clarity_enabled` | Bool |
| `get/setVOICECLARITY_LEVEL` | get/setProperty | headphones | `voice_clarity_level` | Float |
| `get/setVOICECLARITY_NOISESUPP_LEVEL` | get/setProperty | headphones | `voice_clarity_nr_level` | Float |

Enums: magic voice `MONSTER_V1 / CARTOON_V1 / MALE_VOICE_V1 / FEMALE_VOICE_V1` (UI 0–3);
adaptive volume `high_dyn | low_dyn`; room `large | medium | small` (UI 0,1,2).
Consequence: **on THX devices the EQ is 10-band gain in dB** — same index model as legacy
`getEQ_Slider`, and the same 10 values as `eq.json`.
The renderer also ships a *"thx-ready"* mock (`THX_MOCK_*`, mock device `"THX Mock Device"`,
version `9999.9999.9999`) implementing this contract for environments without the THX runtime —
usable as behavioural test spec.

---

### 7.13 Factory-reset / canonical default values `[VERIFIED from otherSetting reset sequence]`

`index-Btnf0C9W.js` (Other settings) implements "恢复出厂/恢复默认" by calling every audio setter
with its default. This is a precise, machine-level statement of defaults for the legacy/C-Media
path (and the values the UI expects on first run):

| Setting | Reset call |
|---|---|
| Speaker mute | `setMuteControl(false, "speaker")` |
| Mic volume scalar | `setVolumeScalarControl(-1, 0.92, "mic")` |
| AI noise reduction | `setAINREnable(false)` |
| Steady-state NR | `setNR_Enable(false)` |
| Magic voice | `setEnable_MAGICVOICE(false)`, `setMagicVoice_Selection(0)` |
| Audio brightening | `setEnable_AUDIOBRILLIANT_LFX(false)`, `setAUDIOBRILLIANT_LEVEL(3)` |
| Surround amplification | `setEnable_Channel_COPY_LFX(false)` |
| Dynamic bass | `setEnable_VIRTUALBASS_LFX(false)`, `setVIRTUALBASS_Level(5)`, `setVIRTUALBASS_CutOffFrequency(200)` |
| Smart volume | `setEnable_ADAPTIVEVOLUME_GFX(false)`, `setADAPTIVEVOLUME_LEVEL(50)`, `setADAPTIVEVOLUME_MODE(0)` |
| Vocal clarity | `setEnable_VOICECLARITY_LFX(false)`, `setVOICECLARITY_LEVEL(3)`, `setVOICECLARITY_NOISESUPP_LEVEL(0)` |
| Mic leveling | `setEnable_MICVOLUMELEVELING(false)`, `setMICVOLUMELEVELING_LEVEL(0.5)` |
| Mic voice clarity | `setMicVoiceClarityEnabled(false)`, `setMicVoiceClarityLevel(0)` |
| Equalizer | `setEnable_EQ_GFX(false)`, `setEQ_Slider(i, 0)` for all bands |
| Xear surround | `setXearSurroundEnable(false)`, `setUpMixEnabled(false)`, `setXearSurroundMode(0)`, `setXearSurroundRoom(1)` (= medium) |
| Friendly name read | `getDeviceFriendlyName("mic")` |

Note the scale difference for THX devices: same defaults exist but THX levels are 0–100 (e.g.
brightening default 0, bass 0.55) — see §7.3; the reset sequence above is the C-Media profile.

---

## 8. `electronAPI` surface (from `asar/out/preload/index.js`) `[VERIFIED, plaintext]`

The preload is readable and is the authoritative IPC contract (415 lines). Grouped
(renderer-side wrapper signatures):

**Meta / window / lifecycle**
`getVersion`, `getEnv`, `getArgv`, `getMainFlags`/`setMainFlags(obj)`, `getIsAdminRunning`,
`isInstalledInSystemDrive`, `restartComputer`, `quit`, `openDevTools`, `getRendererBuildInfo`,
`getCurEnv`/`setCurEnv`, `getTargetEnvConfig(p)`, `getEnvConfigs`, `reportRendererReady`,
`getRendererUrl`, `getRefreshPageContext`, `refreshGlobalConfig(options)`,
`probeRendererReachable(target,options)`, `retryLoadRenderer(options)`, `checkVcRedistX64`,
`syncDeviceConfigToMainProcess(deviceConfigMap)`,
`closeWindow`, `hideWindow`, `minimizeWindow`, `maximizeWindow`, `unmaximize`, `toggleMax`,
`isMaximized`, `isWindowVisible(id)`, `getWindowStateSnapshot(id)`, `getWindowSize`,
`setWindowSize(w,h)`, `setWindowMInSize(w,h)`, `setZoomFactor(f)`, `setBounds(x,y,w,h)`,
`getDragBoundsAndRestoreIfMaximized(x,y)`, `setPosition(x,y)`, `getScreenInfo`, `getWindowsTheme`,
`getThemeValue`, `windowManager(params)`, `createTrayWindow(params,useOldTray)`, `customTray(params)`.

**Config / logs / locale**
`logInfo/logWarn/logError(msg)`, `getConfig(name)`/`setConfig(name,data)`,
`getLang`/`setMainLang(lang)`/`getDefaultLang`, `getSharedConfig`/`setSharedConfig(p)`.

**KV store** (`files/config/mc_main_store_key.json`)
`getAllStore()`, `watchAllStore()` → `{success,listenerId}`, `unwatchAllStore(id)`,
`setStore(key,value,persistent=false)`, `getStore(key,defaultValue=null)` → `{success,value}`,
`hasStore(key)`, `deleteStore(key)`, `clearStore()`; event **`storeChange`**;
`getUserConfig()`/`setUserConfig(config)` (cross-origin login state).

**Legacy migration**
`getLegacyRendererLocalStorage()`, `getLegacyHttpsLocalStorage()`, `getLegacyHttpsIndexedDB()`,
`migrateLegacyUrlIndexIframeStorage()`, `getLegacyK20GtCustomImageHistory()`.

**Files / images / background**
`readLocalFile(path)`, `openUserFilesDir()`, `deleteFile(p)`, `showOpenFileDialog(p)`,
`importData()`, `exportData(data,defaultPath)`, `saveBgImg(from)`, `getBgImg()`, `saveGifImg(p)`,
`getGifImgList()`, `showWinToast(p)`, `localFiles.{getRoot,read,write,delete,exists,mkdir,list}`
(relative to `files/local`), `installDriver(p)`, `uninstallDriver()`, `setToDefault()`.

**System integration**
`setAutoStart(v)`/`getAutoStartStatus()`, `openSoundSetting`, `openScreenSetting`,
`openWindowsMouseSetting`, `openBrowser(url)`, `getAppMetrics()`.

**Audio SDK (C-Media)**
`AudioSDKGetState()`, `AudioSDKInstallDriver(options)`, `AudioSDKReleaseDevice(deviceId)`,
`AudioSDKEnsureReady(registration)`, `AudioSDKCall(method,args=[])`, `initDll(name)`.

**Audio runtime THX**
`AudioTHXGetState`, `AudioTHXGetInstallState`, `AudioTHXGet7TestVideoInfo`,
`AudioTHXListAudioPlayerProducts`, `AudioTHXListAudioPlayerEndpoints`,
`AudioTHXResolveAudioPlayerEndpointGuid`, `AudioTHXAudioPlayerControl(request)`,
`AudioTHXDownloadInstaller`, `AudioTHXGetDownloadInstallerState`, `AudioTHXInstallInstaller`,
`AudioTHXUninstallBundle`, `AudioTHXGetRecoveryState`, `AudioTHXCheckAfunix`,
`AudioTHXRunServiceRepair`, `AudioTHXEnsureReady(options)`, `AudioTHXCall(request)`,
`AudioTHXReleaseDevice(request)`, `AudioTHXReleaseClientShellByHwid(request)`,
`AudioTHXClearAudioPlayerEndpointByHwid(request)`, `AudioTHXSubscribePropertyChanges(request)`,
`AudioTHXUnsubscribePropertyChanges(request)`.

**Speaker / mic / effects (AudioSDK path)**
`get/setVolumeScalarControl(channel,val,type)`, `get/setMuteControl(muted,type)`,
`getAINREnable`/`setAINREnable`, `getNR_Enable`/`setNR_Enable`,
`getEnable_MAGICVOICE`/`setEnable_MAGICVOICE`,
`getMagicVoice_Selection`/`setMagicVoice_Selection(type)`,
`getEnable_AUDIOBRILLIANT_LFX`/`setEnable_AUDIOBRILLIANT_LFX`,
`getAUDIOBRILLIANT_LEVEL`/`setAUDIOBRILLIANT_LEVEL`,
`getEnable_Channel_COPY_LFX`/`setEnable_Channel_COPY_LFX`,
`getEnable_VIRTUALBASS_LFX`/`setEnable_VIRTUALBASS_LFX`,
`getVIRTUALBASS_Level`/`setVIRTUALBASS_Level`,
`getVIRTUALBASS_CutOffFrequency`/`setVIRTUALBASS_CutOffFrequency`,
`getEnable_ADAPTIVEVOLUME_GFX`/`setEnable_ADAPTIVEVOLUME_GFX`,
`getADAPTIVEVOLUME_LEVEL`/`setADAPTIVEVOLUME_LEVEL`,
`getADAPTIVEVOLUME_MODE`/`setADAPTIVEVOLUME_MODE`,
`getEnable_VOICECLARITY_LFX`/`setEnable_VOICECLARITY_LFX`,
`getVOICECLARITY_LEVEL`/`setVOICECLARITY_LEVEL`,
`getVOICECLARITY_NOISESUPP_LEVEL`/`setVOICECLARITY_NOISESUPP_LEVEL`,
`getXearSurroundEnable`/`getXearSurroundRoom`/`getXearSurroundMode` + setters,
`playTestSound(value)`/`stopTestSound()`, `GetEnable_EQ_GFX`/`setEnable_EQ_GFX`,
`getEQ_Slider(index)`/`setEQ_Slider(index,value)`.

**Mic listen / volume watch**
`getMicListenStatus`/`setMicListenStatus(p)`, `startWatchAudioVolume`/`stopWatchAudioVolume`,
`getAudioVolume(p)`/`setAudioVolume(p)`, `getAudioMute(p)`/`setAudioMute(p)`.

**Device / firmware / OTA**
`getFirmwareVer()`, `getDeviceFriendlyName(type)`, `getFirmwareBinData(p)`,
`getUsbDeviceDescriptor(p)`, `getFWVersionFromUsbDescriptor(p)`, `getUsbStringDescriptor(p)`,
`getUsbVersion(p)`, `otaDevice(p)`, `download(version,urlUpdate)`, `recorderCall(method,param)`.

**Apps/games/lyrics** — see §7.11; plus `removeInfLinkPlugin()`, `installBetterNcm()`,
`uninstallBetterNcm()`.

**Updater**
`updater.getState()`, `updater.startSilentDownload(params)`, `updater.startManualUpdate(params)`,
`updater.applyDownloadedUpdateNow()`, `updater.onStateChanged(cb)` → `updater:stateChanged`.

**Events (registered via `addEventListener(event,key,cb)`)**
`callback_VOLUMEANDMUTE`, `updateError`, `onDllReady`, `onRecorderData`, `onRecorderError`,
`onAudioVolumeChange`, `onActiveAppChange`, `onLyricsMediaUpdate`, `betterNcmInstallProgress`,
`storeChange`, `onOtaMessage`, `onIframeLoadFailed`, `windowStateChange`,
`onTHXPropertyChanges`, `onTHXPropertyChangesEnded`, `onTHXPropertyChangesError`;
direct listeners: `battery_info` (`onBatteryInfo`), `maximize`, `unmaximize`, `downloadProgress`.

### 8.1 How UI panels reach the audio layer `[VERIFIED]`

UI uses a device proxy `p.get(method, args, default)` / `p.set(method, args)`; dispatch:

| runtime | dispatch |
|---|---|
| `cmedia` | `electronAPI.AudioSDKCall(method, [deviceId, ...args])` → `{success, data}` (wrapper logs `AudioRuntime <method> 调用失败`) |
| `thx` | legacy name mapped (§7.12) → `electronAPI.AudioTHXCall({productKey, method, args})` |

Because `AudioSDKCall` accepts arbitrary method names, some UI methods have **no dedicated preload
wrapper** (`setUpMixEnabled`, `getUpMixEnabled`, `getAINRLevel`, `setAINRLevel`,
`getDefaultDeviceControl`, `getDeviceFriendlyName`). The proxy names in the panels are the contract;
§8 is the convenience surface.

---

## 9. Data schemas

### 9.1 Store (`%APPDATA%\MCHOSEHUB\files\config\mc_main_store_key.json`) `[VERIFIED]`

Top-level keys: `user_config`, `net-config-global`, `rendererVersion`,
`legacy_file_iframe_urlindex_storage_migrated_origins`, `webIndexListLen`, `webIndexList`.

`webIndexList[i]` — connected-device object (V9 PRO observed):

```jsonc
{
  "vendorId": 10525, "productId": 14429,
  "productKey": "MCHOSE V9 PRO$$$10525$$$14429",
  "deviceName": "V9 Pro", "productName": "MCHOSE V9 PRO",
  "colorList": [ {"color":"color-white","pic":"…/headset/V9Pro_whiteSilver.png"}, … 6 variants ],
  "checkIndex": 0, "lockIndex": 3,
  "deviceType": 3,                       // 1 keyboard, 2 mouse, 3 audio
  "batteryLevel": null, "connectMode": 1, "showBattery": true,
  "enableBatteryVersion": "0008", "isElectronDevice": true,
  "detailInfo": {"productKey":"MCHOSE V9 PRO","wiredKey":"wireless",
                 "productNameOriginal":"MCHOSE V9 PRO","productId":14429,"vendorId":10525},
  "storageKey": "MCHOSE V9 PRO$$$10525$$$14429",
  "isBusy": false, "profile": {"index":null,"detail":[],"order":[]},
  "deviceGroup": "", "communicateEnabled": true, "setProfileIndex": null, "getFwInfo": null,
  "upgradeStatus": "", "isUpgradeMode": false,
  "batteryPrecision": 10, "allowUpdateBattery": false, "firstGuideType": "disabled",
  "needRemoveListeners": [null], "isSleepElectron": false, "chargeStatusElectron": null,
  "isP1": true,
  "firmware": {"headsetVersion":"0024","dongleVersion":"0004","speakerVersion":null}
}
```

`net-config-global` = the remote global config; `rendererVersion` = version object (same as CDN).

### 9.2 `eq.json` `[VERIFIED]`

See §7.4(a); `current` maps `"<product name> PID<pid>"` → preset name.

### 9.3 `system.json` `[VERIFIED]`

```jsonc
{ "firstRun": false, "autoStart": true, "autoUpdate": false, "autoStartMini": true,
  "theme": "light", "hideWhenClose": true,
  "bgImgMode": "auto", "bgImgModeL2": "customize_default_1", "bgImgPath": "",
  "bgImgBlur": 0, "bgCardBlur": 40, "uiScaleLevel": "default", "lang": "en_US",
  "bgWallpaperV2": {"schemaVersion": 2, "wallpaperId": "official-solid"},
  "bgImgBrightness": 50 }
```

### 9.4 Remote global config (`configCenter-activeValue.json`) `[VERIFIED]`

`customFields{headerBtnConfigs, expertViewConfig, bgConfigV2, bgConfig, gtConfig, isRemoteLang,
firstGuide}`, `version`, `pcVersion`, `webVersion`, `refreshTime`, `rendererVersion`.

`version` = **device config version table**, key `"<product name>--<vidDec>--<pidDec>"` → short hash,
incl. `"MCHOSE V9 PRO--10525--14429": "57db35ba97"`, `V9 Turbo+--14391--24586`,
`V9 Turbo--14391--24589`, `K20 GT--14391--24774`, `V9 PRO 2--14391--24592`,
`V9 Pro 2--14391--24593`, `R9--14391--24594/24595`, `G9 V2--14391--24591`, plus feature entries
(`keyboardPreset`, `keyboardConfig`, `mouseConfig`, `newMouseConfig`, `cardList`,
`otaConfigApp_prod`, `audition`, `固件库`, `mchosePcFile`, …).

`firstGuide` holds per-device onboarding (device keys `"<name>$$$<vid>$$$<pid>"`, `ui:"old"|"new"`,
CN/EN images or `{img,title,des}` triples). R9 copy confirms features: "THX 空间音效 / 自定义EQ /
云端社区EQ共享 / 智能记忆调音，3组音效自由切换".

### 9.5 Firmware minimum-version table `[VERIFIED in main bundle]`

`{productKey, name, versionDongle, versionHeadset}` entries, e.g.
`MCHOSE V9 PRO` / `MCHOSE V9 PRO PID14429` → dongle `0008`, headset `0029`.
The connected unit reports dongle `0004`, headset `0024` → **below minimum** → the UI's firmware
upgrade prompt / red dot is expected.

### 9.6 Device configuration table `[VERIFIED, partially extracted]`

Built by a factory function in `main-xA7mzCYt.js`; key = product key (name + `" PID" + pid when
needed). Fields: `productKey`, `wiredKey` (`wired`|`wireless`), `renderName`, `productBgImg`,
`indexLightbg`, `pdfSource` or `pdfSourceByLang{zh_CN,en_US}`, `audioRuntimeType`
(`cmedia`|`thx`), `driverDir`, `GUID{Data1..Data4[8]}`, `THXConfig{productId,
upgradeCodeBundle, relatedUpgradeCodeBundles[], hwid}`, `isNewLayout`.

Connected device entry:

```jsonc
"MCHOSE V9 PRO PID14429": {
  "productKey": "MCHOSE V9 PRO PID14429", "wiredKey": "wireless", "renderName": "V9 Pro",
  "pdfSource": "V9.pdf", "audioRuntimeType": "cmedia", "driverDir": "S9_PRO_14429",
  "GUID": { "Data1": 430567137, "Data2": 10979, "Data3": 17602,
            "Data4": [152,161,1,147,36,188,6,8] }
}
```

THX example `"MCHOSE V9 PRO 2"`: `audioRuntimeType:"thx"`,
`THXConfig{productId:"mchose_v9pro2", upgradeCodeBundle:"6A738E2C-7335-B56D-5E9A-B28B495EC9B1",
relatedUpgradeCodeBundles:["6A738E2C-…","C8FD213B-FD71-B8A7-556C-16C9A43B42FF"],
hwid:"VID_3837&PID_6011&MI_00"}`, `isNewLayout:true`.
Raw excerpt saved at `renderer/notes/device-config-table.txt`.

Capability lists (main): `HQ.productIds = [14429,273,32896,24772,24645,24586,24584,24579,24589,
24593,24774]`, `HQ.vendorIds = [10525,3690,14391]`,
`HQ.names.headset = ["S9 Pro","G9 Pro","V9","V9 Pro","X9","V9 TURBO+","V9 TURBO HEADSET","V9 Turbo",
"V9 TURBO","V9 PRO 2"]`, `HQ.names.speaker = ["G20","G20 Pro","K20"]`.

### 9.7 Remote "custom" config set (`configCenter/custom/*.json`) `[VERIFIED, decoded]`

Fetched as `<file>?hash=<short hash from global config version table>`:

| file | hash | decoded | content |
|---|---|---|---|
| `cardList.json` | `0984e23f7f` | 43 184 B | homepage device-card catalogue (22 entries) |
| `keyboardConfig.json` | `1c85622bab` | 5 870 B | per-device keyboard configs keyed `0x3837_0x3033_MCHOSE G87 V2 2.4G` … |
| `keyboardPreset.json` | `9c6d7b83ef` | 7 938 B | presets keyed `0xVID_0xPID:index` |
| `mouseConfig.json` | `89b67ad5c0` | 1 392 B | mouse configs keyed by PID / model |
| `newMouseConfig.json` | `6817376c69` | 7 455 B | new-generation mouse configs |
| `pre-newMouseConfig.json` | `58bf0d7bae` | 8 307 B | staging variant |

**Encoding**: `{"__compressBase64__": "<base64>"}` where base64 = **ZIP containing `data.json`**
(renderer: `new JSZip().loadAsync(value,{base64:true})` then `file("data.json").async("string")`).
Decoder: `scripts/decode-configs2.js` → `renderer/site/configCenter/custom/decoded/*.decoded.json`.
(A `mchose_secret_2024` XOR helper exists in the bundle but is not used on this path.)

`cardList` entry schema (home/connect cards):

```jsonc
{ "desc": "Ace 75 磁轴键盘（8K 已发布, 16K 未发布）",
  "identities": [{"vendorId":"0x3837","productId":"0x303c","usagePage":"0x0001",
                  "usage":"0x0000","priority":1}, …],
  "colorList": [{"className":"color-ace75-white","image":"…png","deviceImage":"…Full….png",
                 "theme":"light","styleRaw":"…"}, …],
  "webdriverEnum": …, "glbModelPath": … }
```

`cardList` covers **keyboards only** (web driver excludes audio). Headset cards come from the
store's `webIndexList` (§9.1) with `configCenter/assets/img/headset/*` images.

---

## 10. Renderer-side HID protocol (WebHID) `[VERIFIED from code]`

Transport (`webHIDBase`): `navigator.hid.getDevices()` filtered by productId+vendorId+productName
(case-insensitive); device opened with `open()`; writes `device.sendReport(reportId, Uint8Array)`;
replies awaited on `inputreport` (matching `reportId` and byte-prefix `cmdStart`), 3 000 ms timeout
returning `false`.

Frame (`crc32-BlQnRCeo.js`, 63-byte report, `reportDataLen = 63`):

| offset | len | field |
|---|---|---|
| 0 | 1 | start `0xAA` (170) |
| 1 | 1 | version `0x01` |
| 2 | 1 | flags (`1` = checksum present) |
| 3 | 1 | length = payloadLen + 1 (includes cmd) |
| 4 | 2 | cmdType pair: Request/Response `[0,1]`, Set `[0,2]`, Notify `[0,4]`, UpImg `[0,3]`, OtaRequest `[0,6]` |
| 6 | 1 | cmd |
| 7 | n | payload |
| 7+n | 1 | checksum |

Checksum = **XOR** of `flags, length, cmdType[0], cmdType[1], cmd, payload[*]` (not CRC32).
Validation: `buf[0]==0xAA`, `length>=1`, bounds check, XOR check when flags==1.
Payload extraction: `slice(6+1, 6+1+length-1)`.

Report ids by device family (observed in code paths): **85 (0x55)** for V9/headset EQ+battery and
new-layout HidEQ, **188 (0xBC)** for K20/K20 Pro/K20 GT, **170 (0xAA)** for G9 V2.
`[85 verified for headset; family list inferred from device filters]`

Notify command map (decoded events):

| cmd | type | payload |
|---|---|---|
| 34 (also 11) | `batterySleep` | `sleepState`(0=asleep), `byte1>>7`=charging, `byte1&0x7F`=percent |
| 1 | `eqMode` | mode index |
| 2 | `voiceBroadcast` | 0/1 |
| 3 | `controlDetail` | `controlMode,brightness,cycle,colorIndex,curColor[3]` |
| 5 | `lightGroupsOn` | `groupsNum,side,mid` |
| 6 | `smartLight` | 0/1 |
| 7 | `controlMode` | mode |
| 8 | `initMsg` | value |
| 9 | `screenState` | `screenSwitch,color[3],mode,curTheme,index` |
| 10 | `themeSwitch` | 9 bytes |
| 11 | `lyricState` | `lyricSwitch,scroll,text(len-prefixed UTF-8)` |
| 14 | `colorMap` | `index,num,color[3]` |
| 15 | `screenTime` | — |
| 29 | `screenTextState` | `testType,align,scroll,length,text` |
| 32 | `exitLyricsDisplay` | screenState-like |
| 35 | `screenOTAState` | bool |
| 38 | `customImageName` | length-prefixed UTF-8 |
| 39 | `lyricSettings` | `lyricSwitch,font,fontSize,align,direction,animation` |
| 40 | `customTextTheme` | `testType,font,fontSize,align,direction,animation,len,text` |

Battery derive: `chargeStatus = charging | full (>=100) | discharge`; `isSleep` → hide level.
HidEQ PEQ structures/commands — §7.4(b).

---

## 11. Audio runtime selection and driver binding `[VERIFIED table + calls; mechanics INFERRED]`

| Runtime | Devices (examples) | Renderer API family | Notes |
|---|---|---|---|
| `cmedia` (AudioSDK) | V9 PRO, V9, V9 TURBO/+, S9 PRO, G9 PRO, G9 V2, G20/G20 PRO, K20/K20 PRO/K20 GT, X9 | legacy `get/set*` LFX/GFX names; `GetEnable_EQ_GFX`, `getEQ_Slider`; Xear surround | requires `AudioSDKEnsureReady` + `AudioSDKInstallDriver`; `driverDir` + device-interface GUID; may need admin (`common.notAdminRunning`, `devicePage.notAdminRunningHeadset`) and reboot (`devicePage.restartNotice`) |
| `thx` (AudioRuntimeTHX) | V9 PRO 2, V9 TURBO+ (capable), R9 | `AudioTHX*` + property contract; on-device PEQ via WebHID; THX spatial page | installs/repairs a THX audio bundle (`upgradeCodeBundle`), afunix check, service repair, hwid binding; "音效组件" dialog in device page |

Both coexist with WebHID device control (battery, PEQ on new layout, lighting, screen).

---

## 12. i18n inventory

Two independent string tables, all 9 locales (`zh_CN, en_US, ru_RU, de_DE, fr_FR, ja_JP, ko_KR,
es_ES, pl_PL`):

1. **Desktop renderer** — from `_plugin-vue_export-helper-Ce2ZIww_.js` →
   `renderer/i18n-prod/{kh,Sh,Ch,Mh,Ah,Eh,Dh,Th,Ph}.json` (`kh`=zh_CN, `Sh`=en_US). Namespaces:
   `common, tray, commonHeader, themeSetting, setting, menus, speakerSettings, micSettings, index,
   devicePage, routine, eq, userProfile, wechatLogin, user, mode, light, surround, thxSurround,
   otherSettings, screenSettings, update, musicDance`. ~1 200 flattened keys; `eq` and
   `screenSettings` are largest.
2. **Browser web driver** — from `theme-IdBKOUK0.js` →
   `renderer/i18n/{vme,mme,yme,wme,bme,Sme,_me,zme,Tme}.json` (`vme`=zh_CN). Namespaces: `common,
   themeSetting, home, connectDevice, mouse, mouseMacro, keyboard, light, menu, gif3dBox, lightSync,
   gifDialog, wechatLogin, setting, autoBoard, cloudCommunity, userProfile, user, tray, testTool,
   satisfaction` (1 210 keys).

Merged helper: `renderer/i18n/_merged.json`. Key namespaces map to screens: `menus` = sidebar,
`devicePage` = device states/OTA/errors, `routine` = audio routine, `speakerSettings`/`micSettings`
= effect cards, `surround`/`thxSurround` = spatial, `otherSettings` = firmware/power/factory,
`eq` = presets + editor, `mode` = game/music/movie modes, `light`, `screenSettings`, `update`,
`musicDance`.

---

## 13. Unknowns and limitations

1. **Main-process specifics** (window creation, `mchose-renderer://` handler, HID permission
   policy, AudioSDK/firmware internals) are not analysed here — main is V8 bytecode; native-re and
   runtime-rec own it. Statements about them are `[INFERRED]`.
2. **Exact Electron→CDN load mechanism** (direct URL vs custom scheme proxy) unproven; cache proves
   content, not mechanism.
3. **Legacy EQ slider range**: presets contain −13…+14, HidEQ/THX clamp ±12; C-Media UI range
   (likely ±12) `[UNKNOWN]`.
4. Some control ranges are still unread line-by-line (mic page clarity/leveling all confirmed
   0..100 in THX; C-Media variants partly). Routine/EQ panel internal state machine partly covered.
5. **HID payload layouts**: all commands the UI actually uses are documented (EQ read/gain writes,
   prompt volume, auto-shutdown, factory reset, mode name/restore, battery). `SET_PEQ_ALL` (cmd 16)
   is declared but unused by this build (§7.4b). Not covered: exact `SET_PEQ_ALL_GAIN` ack handling
   edge cases and the k20/other-device report-188/170 payloads (out of headset scope).
6. `isNewLayout` for the connected V9 PRO: device table has no flag → legacy expected; runtime DOM
   capture should confirm.
7. Visual design tokens (colours, spacing, blur) only partially derivable from CSS; **no usable
   screenshots yet** (§13.1).
8. `refresh.html`/`tips.html` inner UI beyond the copy keys not documented.
9. Cloud/account backends (share/favourites/review) visible only as i18n + few URLs; request/response
   shapes unknown (not needed for offline core).

### 13.1 Screenshot attempts (negative results, documented)

Three controlled attempts with Edge 155 headless (via REA `capture_browser_scenario` and a
hand-written CDP driver):

1. **Renderer URL in a plain browser** (`…/customPage/prod/index.html?v=1.1.26`): bundles load
   (HTTP 200s recorded) but the page is blank; console
   `[uiScale] init shell scale failed: DesktopError: 当前环境不支持 Desktop capability：getConfig`
   and page error `Cannot read properties of undefined (reading 'getConfig')`.
   → the renderer hard-depends on `window.electronAPI`.
2. **Browser web driver** (`https://www.mchose.com.cn/`): boots (`CZ initialized`,
   `support OffscreenCanvas`) then aborts with
   `wrapRemoteConfig::> keyboard config not found, check url: …/keyboardConfig.json` (that run's
   request carried no `hash`), leaving a blank page. The same configs load fine with `?hash=`
   (verified by direct fetch, §9.7), so this is a config/caching condition in headless, not proof
   the app is broken.
3. **Local mock-preload harness** (`renderer/harness/`: `index.html` + `preload-stub.js` +
   `global-config.js` + `server.js`): defines a mocked `electronAPI` (store/config/env/window/audio;
   real V9 PRO device object, real `system.json`, real global config) and imports the remote
   renderer bundle. Result: **the real renderer boots outside Electron** (Vue app initialises,
   i18n → `zh-cn`, store watcher starts, updater container mounts, legacy migration runs), but it
   does not paint: remaining failures are capability-contract gaps, e.g. `加载全局配置失败: Cannot
   read properties of undefined (reading 'configCenterBaseUrl')`, `… (reading 'apiConfig')`
   (HTTP base URL from the env store), `启动 Store 监听器失败: undefined`, plus an OTA-config CORS
   block. Artifact `renderer/shots/harness-index.png` is a blank frame kept as attempt evidence.

Recovered env-config contract (production values, from `_plugin` `Ft`/`Fh()`):

```jsonc
{ "urlIndex": "https://www.mchose.com.cn/#/connectDevice",
  "urlTray": "https://www.mchose.com.cn/tray.html",
  "urlRenderer": "https://cdn.mchose.com.cn/customPage/prod/index.html",
  "urlUpdate": "https://www.mchose.com.cn/update/",
  "appName": "MCHOSE HUB",
  "configCenterBaseUrl": "https://cdn.mchose.com.cn/configCenter",
  "wechatRedirectUri": "https://www.maicong.cn",
  "apiConfig": { "baseUrl": "https://www.maicong.cn/mchose", "timeout": 20000,
                 "retryCount": 1, "enableLog": false } }
```
Environments `dev` / `pre` (`customPage/pre/index.html`, `drivertest.maicong.cn`) / `production`;
selected by `getCurEnv()` / `getRendererBuildInfo().buildEnv`.

**Recommendation**: obtain UI screenshots from the *running* installed app (runtime-rec via CDP
`Page.captureScreenshot`), not by re-hosting.

---

## 14. What the Rust/egui reimplementation should reproduce (headset-first)

Priority 1 — must have (parity for the connected V9 PRO, C-Media/legacy path):

1. **Home / device discovery**: device list (VID/PID + product name), battery, connection mode
   (2.4G/wired), upgrade badge, first-guide, offline/loading/refresh/error states, sleep mask.
2. **Device shell**: fixed header (title, theme, settings, window controls), left sidebar with
   red-dot firmware badge, content per route.
3. **Audio routine page** (`routineLegacy` composition §7.1): hero device image; column 1 volume +
   mic; column 2 surround amplification + dynamic low frequency; column 3 smart volume + vocal
   clarity; plus voice-prompt volume, AI-NR/noise-suppression, magic voice, mic-listen.
4. **10-band EQ page**: 10 sliders/curve, preset library from `eq.json` (factory game/music groups),
   save/rename/delete, enable switch, per-device current preset, import/export JSON.
5. **Virtual 7.1 page**: on/off, music/game vs movie mode, room size, speaker test.
6. **Other settings**: dongle/headset versions, update prompt + progress, auto-shutdown minutes
   (never + presets), device power-off, factory reset with confirm, manual link, language.
7. **Persistence parity** with `files/config/*.json` shapes (§9.1–9.3) and store semantics.

Priority 2 — shell/secondary:

8. Theme (light/dark/follow-system), wallpaper + custom image + blur, UI scale, 9 languages
   (zh_CN + en_US strings are extracted here).
9. Settings page (auto-start, start minimised, close behaviour, version).
10. Sound modes (`audio_mode`) and routine multi-mode concept where supported.
11. Tray menu (open/quit).
12. Keyboard/mouse pages — either reimplement the web app's screens (§4.2 routes,
    `renderer/i18n/` strings, `cardList` catalogue) or keep an embedded browser path.

Priority 3 — networked extras (optional): cloud EQ community, account binding, OTA downloads,
lyrics/music integration, screen/custom images, AI image generation, app/game auto-switch.

### 14.1 Scope note

The original client natively implements only `/themeSetting` and `/headset/*`; the device
list/connect "home" (`/index`) and all keyboard/mouse device UIs are the **browser web driver inside
an iframe** (`urlIndex` / `iframeBridge`). Consequently:
- headset parity = the 14 `/headset/*` screens + theme + window/tray/updater chrome;
- "home" parity for a Rust rewrite can be either (i) a native reimplementation of the web driver's
  `connectDevice` page (routes §4.2, `connectDevice.*` strings in `renderer/i18n/`, device catalogue
  from the store + `cardList`), or (ii) an embedded webview pointing at the same URL — the second is
  closer to the original and much cheaper.

### 14.2 Recommended Rust architecture (derived from the code)

```rust
enum Route { Index, ThemeSetting, Headset(HeadsetTab) }
enum HeadsetTab { AudioRoutine, AudioEq, AudioMode, AudioSurround, AudioSurroundThx,
                  AudioSpeaker, AudioMic, Lighting(LightingKind), ScreenSetting, OtherSetting }

struct AppState {                       // mirrors the Pinia stores
    devices: Vec<Device>,               // webIndexList objects (§9.1)
    selected: Option<usize>,
    route: Route,
    store: Store,                       // getStore/setStore/watch semantics + files/config/*.json
    system: SystemConfig,               // §9.3
    global: GlobalConfig,               // §9.4 (versions, wallpapers, firstGuide)
    theme: Theme, lang: Lang,
}

trait AudioBackend {                    // one UI, two runtimes (§7.12)
    fn get_property(&mut self, ep: Endpoint, id: &str) -> Result<Value>;
    fn set_property(&mut self, ep: Endpoint, id: &str, v: Value) -> Result<()>;
    fn get_param(&mut self, ep: Endpoint, id: u16) -> Result<Value>;
    fn set_param(&mut self, ep: Endpoint, id: u16, v: Value) -> Result<()>;
    fn select_preset(&mut self, group: PresetGroup, id: &str) -> Result<()>;
    fn play_test_sound(&mut self) -> Result<()>;
    fn stop_test_sound(&mut self) -> Result<()>;
}
struct CMediaBackend;                   // legacy names → C-Media driver calls (as today's main proc)
struct ThxBackend;                      // legacy names → THX property API (adapter map §7.12)

trait HidTransport {                    // WebHID equivalent (report 85 family)
    fn send(&self, report_id: u8, data: &[u8]) -> Result<()>;
    fn read_notify(&mut self) -> Result<(u8, Vec<u8>)>;   // battery / mode / lighting
}
```

Per-screen widgets map 1:1 to the Vue components (`renderer/notes/components-out.json` lists every
`__name` per chunk). The PEQ chart editor can mirror `hidEqChartOptions-*.js` (ECharts options =
axis/bands/series description). HID frame codec from §10 is small enough to port directly.

---

## 15. Appendix — artifacts produced by this task

```
renderer/
  site/                  # browser web driver (v3.16.32) + configCenter loader + decoded global config
  site/configCenter/custom/decoded/   # decoded cardList/keyboardConfig/keyboardPreset/mouseConfig/…
  prod/                  # Electron renderer customPage/prod build (64 files) + _crawl.json (sha256)
  pretty/                # beautified web-app bundles
  pretty/prod/           # beautified renderer bundles (main, headsetPage, _plugin, eq, crc32, pages…)
  i18n/                  # web-app locales (9) + _merged.json
  i18n-prod/             # desktop-renderer locales (9)
  cache-bodies/          # decompressed Chromium cache entries + _bodies.json + _blockfile-urls.json
  notes/                 # device-config-table.txt, audio-method-map.json, components/api inventories
  harness/               # mock-preload harness (index.html, preload-stub.js, global-config.js, server.js)
  shots/                 # harness-index.png (blank; attempt evidence)
  scripts/               # analysis tooling (§2.4)
```

Reproduce key steps:

```powershell
node renderer/scripts/crawl2.js https://cdn.mchose.com.cn/customPage/prod/ renderer/prod "./index.html?v=1.1.26"
node renderer/scripts/cache-extract.js "$env:APPDATA/MCHOSEHUB/Cache/Cache_Data" renderer/cache-bodies
node renderer/scripts/map-cache.js renderer/cache-bodies renderer/site renderer/prod   # identity proof
node renderer/scripts/extract-objects.js renderer/prod/assets/_plugin-vue_export-helper-Ce2ZIww_.js kh,Sh,Ch,Mh,Ah,Eh,Dh,Th,Ph renderer/i18n-prod
node renderer/scripts/extract-audio-map.js                                            # audio adapter table
node renderer/scripts/decode-configs2.js                                              # remote custom configs
```
