# Prior art: MCHOSE V9 Pro HID protocol and companion app (recovered)

**Status:** reconstruction of two deleted projects from the user's own Claude Code
sessions, with every protocol claim labelled by how it was established
(*hardware-observed*, *recovered from the vendor bundle*, *native static
evidence*, or *inferred*).

**Date of reconstruction:** 2026-10-20.
**Recovered source:** `prior-art/mchose-web/` and `prior-art/mchose-v9pro-rs/`
(see `prior-art/*/PROVENANCE.md` and `prior-art/recovered-summary.json`).

---

## 0. TL;DR — the eight things that matter

1. **Device:** MCHOSE V9 Pro gaming **headset** + 2.4 GHz receiver. USB composite
   `VID 0x291D / PID 0x385D`, serial `0123456789AB`, `bcdDevice 0x0004`.
   MI_00 = HID with **six top-level collections**; MI_01 = C-Media USB audio
   (`BLK2025GHS`). Chrome/WebHID exposes this as **one `HIDDevice` with all six
   collections**, not one device per collection.
2. **Three vendor HID channels:** command `0xFF82` (input **65**, output **65**),
   notify `0xFF90` (input **85** only), firmware feature `0xFF22`
   (feature **170**). Two consumer-control collections (`0x000C`, reports 6 and
   237) and telephony (`0x000B`, report 7) complete the map.
3. **Frame format** (vendor `buildReportData`): `AA | ver=01 | flags | length |
   cmdType[2] | cmd | payload… | checksum`, 63 data bytes + report ID.
   `length = payload.len + 1`. Checksum = XOR of flags, length, cmdType[0],
   cmdType[1], cmd and payload bytes — **only when `flags & 1`, and the vendor
   sends everything with `check=false`**, so in practice flags/checksum are 0.
   Reply matcher compares **bytes 4,5,6** (`cmdTypeOffset = 4`) with
   `cmdType[0], cmdType[1], cmd`.
4. **CmdType:** Request/Response `[0,1]`, Set `[0,2]`, Notify `[0,4]`,
   UpImg `[0,3]`, OtaRequest `[0,6]`.
5. **EQ opcodes (vendor `EQ_HID_CMD`)** — 1 `SET_EQ_MODE`, 4 `GET_EQ_STATE`,
   5 `GET_EQ_MODE_DESC`, 11 `GET_BATTERY_SLEEP`, 12 `GET_CURRENT_EQ_MODE`,
   13 `GET_PROMPT_VOLUME`, 14 `GET_AUTO_SHUTDOWN`, 16 `SET_PEQ_ALL`,
   18 = the opcode the gain setter **actually sends**, 24 set mode name (≤54
   UTF-8 bytes), 26 `SET_AUTO_SHUTDOWN`, 27 `FACTORY_RESET`,
   33 `SET_PROMPT_VOLUME`. PEQ gains: payload `[mode, count, hi, lo, …]`,
   **int16 big-endian in 0.1 dB**, UI clamp ±12 dB.
6. **Battery** is *not* framed: vendor sends `[0x65, 0x01, 0x00 ×61]` and matches
   an input report starting with `0x65`, reading `level = data[1]`,
   `status = data[2]` (2 discharge, 3 charging, 4 full, 26 sleep).
   **The vendor sends that request on report ID 85** (`reportId` =
   `V9pro2ReportId` = 85), not on the 0xFF82 command collection's 65.
7. **Firmware** is feature report 170: send 63 bytes `[0x01, target, 0×61]`
   (target 1 = dongle, 0 = headset), wait ~300 ms, then `receiveFeatureReport(170)`
   and accept a reply starting `[170, 1, d, d, d, d]` (four ASCII digits in
   bytes 2..6).
8. **The commands were never answered on this unit.** Browser: 10/10 silent
   (framed cmd 12 and raw 0x65 across reports 7/65/85/237/255) while the input
   listener was proven alive. Native Rust: `HidD_SetOutputReport` on 65 →
   **Win32 error 31 (device STALL)**, report 85 → **error 1 (INVALID_FUNCTION)**,
   `HidD_SetFeature(170)` → **error 1**; hidapi `WriteFile` was *accepted* with no
   reply. The vendor's own APO for this product (`CmApoIce.dll`) imports no HID
   write API at all — its only HID code reads indexed string 50 and compares it.

---

## 1. What was recovered, and how far to trust it

| Tree | Files | Verification |
|---|---|---|
| `prior-art/mchose-web/` | 11 files, ~135 KB: `README.md` (protocol reference), `ANALISE-V9-PRO.md` (pt-BR synthesis), `src/protocol.js`, `src/devices/v9pro.js`, `src/hid-decode.js`, `src/transports/webhid.js`, `src/transports/webusb.js`, `demo/index.html`, `serve.cjs`, `package.json`, `test/protocol.test.js` | `node test/protocol.test.js` → **all assertions passed** |
| `prior-art/mchose-v9pro-rs/` | 24 files, ~137 KB: `Cargo.toml`, `src/protocol.rs`, `src/transport.rs`, `src/device/v9pro.rs`, `src/worker.rs`, `src/app.rs`, `src/ui/*`, `src/models.rs`, `src/presets.rs`, `src/raw_hid.rs`, `src/raw_usb.rs`, `src/win_raw.rs`, `assets/presets.json`, `docs/ANALISE-SONDA.md`, `README.md`, `99-mchose.rules` | `cargo check` **exit 0**, `cargo test` **19/19 pass** |
| `prior-art/read-artifacts/` | 16 consumed artifacts recovered from `Read` results | includes the **verbatim vendor cloud preset JSON** |
| `prior-art/evidence/` | human turns, shell output, curated REA evidence, vendor-bundle snippets | raw material for every claim below |
| `prior-art/verification/` | build/test logs, preset comparison | see [`VERIFICATION.md`](../prior-art/verification/VERIFICATION.md) |

Reconstruction method (two independent lanes, reconciled): Claude Code keeps a
content-addressed backup of every file it edits under
`%USERPROFILE%\.claude\file-history\<session-id>\<hash>@v<N>`, and the session
JSONL maps tracked paths to those backups. That lane gives the exact bytes. The
second lane replays every recorded `Write`/`Edit`/`MultiEdit` call. When the two
disagree the newer one wins; the losing copy and the diff are kept under
`prior-art/_replay/`, `prior-art/_stale-file-history/` and
`recovered-summary.json → disagreements`.

**Known gaps** (full list in each `PROVENANCE.md`):

- `ANALISE-V9-PRO.md` and the static server have **no** file-history backups —
  they are replay-only (unverifiable by lane A). The server was written as
  `serve.js` and renamed to `serve.cjs` by a recorded `mv` (web transcript L687,
  2026-10-13T18:13:13Z); it is saved here under the final name. The file-history
  tracker later recorded **22 versions** of it with no persisted bytes, so its
  post-rename content cannot be checked — the recovered 1.5 KB version is the one
  the recorded `Write` produced and it matches the server output captured in the
  session (`MCHOSE V9 Pro driver demo → http://localhost:5173/`, HTTP 200 for all
  demo assets).
- Three `Edit` calls were refused by the real tool (`tool_result` error) and are
  therefore skipped, not applied.
- `demo/index.html`, `test/protocol.test.js`, `README.md`, `src/worker.rs` and
  `docs/ANALISE-SONDA.md` differ between lanes; the newer lane was used and the
  diff recorded (see `recovered-summary.json → disagreements`).
- The Rust project's initial git commit (`b393c4d` per memory) is gone; no git
  objects survived.

---

## 2. Device identity and the HID report map (hardware-observed)

Observed through Windows PnP + `HidP_GetCaps`/`HidP_Get*Caps` and, independently,
through Chrome's `HIDDevice.collections` report lists (the user pasted these logs
into the session — `evidence/human-turns-web.md`, transcript L763, L876, L1027,
L1143):

```
USB composite   VID 0x291D (10525)   PID 0x385D (14429)   serial "0123456789AB"
├── MI_00  HID — six top-level collections
│     Col01  usagePage 0x000C usage 0x01  consumer control  in 2     out 16    feat 16
│     Col02  usagePage 0xFF22 usage 0x01  vendor            in 0     out 0     feat 64
│     Col03  usagePage 0x000C usage 0x01  consumer control  in 3     out 0     feat 0
│     Col04  usagePage 0x000B usage 0x01  telephony         in 3     out 3     feat 0
│     Col05  usagePage 0xFF90 usage 0x01  vendor            in 64    out 0     feat 0
│     Col06  usagePage 0xFF82 usage 0x01  vendor            in 64    out 64    feat 0
└── MI_01  C-Media USB audio  (INF: USB\VID_291D&PID_385D&MI_01, driver BLK2025GHS)
```

**Runtime report map** — the descriptor-declared report IDs, as Chrome lists them
(`usagePage → input / output / feature`):

| usagePage | input | output | feature | Role as used by the app |
|---|---|---|---|---|
| `0xFF82` | 65 | 65 | — | command channel (the only bidirectional vendor collection) |
| `0xFF90` | 85 | — | — | notifications (battery/state pushes) |
| `0xFF22` | — | — | 170 | firmware version |
| `0x000C` | 237 | 237, 255 | 237 | consumer (volume-wheel candidate) |
| `0x000C` | 6 | — | — | consumer (multifunction button → Play/Pause) |
| `0x000B` | 7 | 7 | — | telephony |

Windows PnP also exposed per-collection interface paths
(`…\VID_291D&PID_385D&MI_00&Col01` … `&Col06`), and a PowerShell enumerator
(`HidD_GetPreparsedData` + `HidP_GetCaps` per collection path, query-only
`CreateFile` access 0 — transcript L543, output at L544) printed:

```
UsagePage Usage In Out Feat VID    PID    Product
0x000B    0x01   3   3    0 0x291D 0x385D MCHOSE V9 PRO
0x000C    0x01   2  16   16 0x291D 0x385D MCHOSE V9 PRO
0x000C    0x01   3   0    0 0x291D 0x385D MCHOSE V9 PRO
0xFF22    0x01   0   0   64 0x291D 0x385D MCHOSE V9 PRO
0xFF82    0x01  64  64    0 0x291D 0x385D MCHOSE V9 PRO
0xFF90    0x01  64  64    0 0x291D 0x385D MCHOSE V9 PRO
```

The first four rows match the anatomy above (byte counts include the report-ID
byte). **The `0xFF90` row contradicts Chrome**, which lists *no* output report for
`0xFF90`; see §8.1 for why that matters and how to settle it.

Notes that cost the earlier project several iterations:

- Chrome returns **one device with six collections**; anything that inspects
  `collections[0]` sees telephony and mis-classifies everything.
- Windows' caps API reports sizes but **no report IDs** for the vendor
  collections (they contain only constant bytes), so the IDs above come from
  Chrome's report lists — i.e. from the HID report descriptor as parsed by a
  browser.
- `mc_main_store_key.json` in `%APPDATA%\MCHOSEHUB\files\config\` records the same
  identity as `{"vendorId": 10525, "productId": 14429}`.
- 64-byte reports = 63 payload bytes + 1 report-ID byte, which matches the
  vendor's `reportDataLen = 63`.

---

## 3. Wire format (recovered verbatim from the vendor bundle)

The primary evidence is a de-obfuscated dump of the production bundle
`purify.es-CSYhaQJg.js` (front-end tag `1.1.24`, commit `af3c384b…`) captured in
`evidence/vendor-bundle-snippets.md`:

```javascript
const co = Object.freeze({Request:[0,1],Response:[0,1],Set:[0,2],
                          Notify:[0,4],UpImg:[0,3],UpImgResponse:[0,3],
                          OtaRequest:[0,6]}),
      rJe=170, cJe=1, dJe=0, uJe=0, hJe=1, ppe=2, G3=3, Zc=4, nU=6, dS=7;
function fpe({check:n, flags:e, length:t, cmdType:i, cmd:s, payload:l}){
  if(!n) return dJe;
  let o=0; o^=e; o^=t; o^=i[0]; o^=i[1];
  const a=[s,...l];
  for(let d=0;d<t;d++) o^=typeof a[d]=="number"?a[d]:0;
  return o
}
function Ed({start:n=rJe, version:e=cJe, cmdType:t, cmd:i, payload:s=[],
            check:l=!1, reportDataLen:o=63}){
  const a=l?1:0, d=s.length+1,
        h=fpe({check:l,flags:a,length:d,cmdType:t,cmd:i,payload:s}),
        p=new Uint8Array(o);
  p.set([n],uJe); p.set([e],hJe); p.set([a],ppe); p.set([d],G3);
  p.set(t,Zc);    p.set([i],nU);  p.set(s,dS);     p.set([h],dS+s.length);
  return p
}
function jR({device:n, reportId:e, reportData:t}){ await n.sendReport(e,t) }
async function Qb({device:n, reportId:e, reportData:t, cmdOffset:i=0,
                   cmdStart:s, timeout:l=3e3}){ /* match reportId === e &&
                   bytes.slice(i).startsWith(cmdStart); 3 s timeout → false */ }
```

Decoded:

| Offset | Field | Meaning |
|---|---|---|
| 0 | `0xAA` (170) | frame start (`rJe`) |
| 1 | `0x01` | protocol version (`cJe`) |
| 2 | flags | bit 0 = checksum present |
| 3 | length | payload bytes **+ 1** (the +1 counts the `cmd` byte) |
| 4–5 | cmdType | `[hi, lo]` pair from `CmdType` (`Zc`/`cmdTypeOffset` = 4) |
| 6 | cmd | opcode (`nU`/`cmdOffset` = 6) |
| 7… | payload | (`dS` = 7) |
| 7 + payload.len | checksum | 0 when the flag is off |

Checksum (only when flags bit 0 set):

```
sum = flags ^ length ^ cmdType[0] ^ cmdType[1] ^ cmd ^ payload[0] ^ … ^ payload[n-1]
```

Verified independently by the recovered JS test suite:
`checksum({check:true, flags:1, length:1, cmdType:[0,1], cmd:12, payload:[]}) === 13`,
and a full `GET_CURRENT_EQ_MODE` frame is exactly
`aa 01 00 01 00 01 0c 00 00 00 00 00 00 00 00 00` (first 16 of 63 bytes).
`SET_EQ_MODE(2)` starts `aa 01 00 02 00 02 01 02 …`.

**Reply matching:** `sendReportOnceSync` resolves with the first `inputreport`
whose `reportId` equals the one used for sending and whose bytes from
`cmdTypeOffset = 4` start with the expected `[...cmdType, cmd]`. Default timeout
3000 ms. For reads the expected prefix is `[...Response, cmd]` = `[0,1,cmd]`; for
`SET_EQ_MODE` the vendor waits for `[...Notify, cmd]` = `[0,4,cmd]`; everything
else is fire-and-forget (`sendReportOnce`).

**WebHID byte conventions** (verified against the spec and against the vendor
app): `sendReport(id, data)` / `sendFeatureReport(id, data)` take the report ID as
a *separate argument*; `inputreport`'s `event.data` **excludes** the ID;
`receiveFeatureReport(id)` may include it, so the driver strips a leading ID byte.
Native (hidapi) is the opposite: the first byte of the buffer is the report ID,
and `HidTransport` prefixes/strips centrally. Both implementations also tolerate
a duplicated leading ID byte (`data[0] == reportId && data[1] == 0xAA`).

---

## 4. Command catalogue

### 4.1 EQ (`EQ_HID_CMD`, sent on the vendor's report ID 85)

Recovered verbatim including the helper that encodes gains
(`evidence/vendor-bundle-snippets.md`, transcript L394, 2026-10-13T17:42:52Z):

```javascript
const V9pro2ReportId = 85, reportId = V9pro2ReportId,
      EQ_MODE = Object.freeze({GAME_1:0, GAME_2:1, GAME_3:2}),
      EQ_HID_CMD = Object.freeze({SET_EQ_MODE:1, GET_EQ_STATE:4,
        GET_EQ_MODE_DESC:5, GET_BATTERY_SLEEP:11, GET_CURRENT_EQ_MODE:12,
        GET_PROMPT_VOLUME:13, GET_AUTO_SHUTDOWN:14, SET_PEQ_ALL:16,
        SET_AUTO_SHUTDOWN:26, FACTORY_RESET:27, SET_PROMPT_VOLUME:33}),
      EQ_UI_GAIN_MIN = -12, EQ_UI_GAIN_MAX = 12;
async function setPEQAllGain({device:S, mode:M, gains:Z}){
  const z = CmdType.Set, U = 18, e1 = [M, (Z==null?void 0:Z.length) ?? 0];
  for(const r1 of Z ?? []){
    const n1 = uiGainToFwGain(r1), i1 = Math.round(n1*10);
    e1.push(i1>>8 & 255, i1 & 255)              // int16 BIG-ENDIAN, 0.1 dB
  }
  const t1 = buildReportData({cmdType:z, cmd:U, payload:e1});
  await sendReportOnce({device:S, reportId, reportData:t1})
}
async function setEQModeName({device:S, mode:M, name:Z}){
  const U = 24, e1 = utf8BytesTruncatedWholeChars(Z, 54), t1 = [M, ...e1];
  /* CmdType.Set */
}
```

| Cmd | Name | Direction / cmdType | Payload → reply |
|---|---|---|---|
| 1 | `SET_EQ_MODE` | Set `[0,2]` | `[mode]`; optional Notify ack `[0,4,1,…]`; modes 0/1/2 = Game 1/2/3 |
| 4 | `GET_EQ_STATE` | Request `[0,1]` | — ; reply layout unknown |
| 5 | `GET_EQ_MODE_DESC` | Request `[0,1]` | `[mode]` → `[mode, …UTF-8, 0]` (name capped to `length-1` bytes; falls back to `eq.gameN` i18n key) |
| 11 | `GET_BATTERY_SLEEP` | Request `[0,1]` | — (notify-side type exists: `BatterySleep` = cmd **34**) |
| 12 | `GET_CURRENT_EQ_MODE` | Request `[0,1]` | `[]` → reply payload `[mode]` |
| 13 | `GET_PROMPT_VOLUME` | Request `[0,1]` | — |
| 14 | `GET_AUTO_SHUTDOWN` | Request `[0,1]` | — |
| 16 | `SET_PEQ_ALL` | declared, **unused** | the real gain setter uses 18 |
| **18** | *(set all PEQ gains)* | Set `[0,2]` | `[mode, count, hi0, lo0, hi1, lo1, …]` int16 BE ×0.1 dB |
| 24 | *(set EQ mode name)* | Set `[0,2]` | `[mode, …UTF-8 name]`, ≤ 54 UTF-8 bytes, whole-character truncation |
| 26 | `SET_AUTO_SHUTDOWN` | Set `[0,2]` | expected `[value]` (not recovered from a call site) |
| 27 | `FACTORY_RESET` | Set `[0,2]` | no payload recovered |
| 33 | `SET_PROMPT_VOLUME` | Set `[0,2]` | expected `[value]` (not recovered from a call site) |

Payloads marked "expected" are inferred from the shared framing, **not** observed.
The gain codec is exact and is the one thing a rewrite should copy verbatim:
`round(clamp(g, -12, 12) * 10)` → two bytes big-endian.

### 4.2 Battery (unframed, vendor report ID 85)

```javascript
async function q7t(n, e=false){                       // getHeadphonesBatteryInfo
  ...
  if (!n.showBattery) return {...};
  const p = n.enableBatteryVersion;
  typeof p == "string"
    ? (p === "0000" ? o = true                              // V9 Pro ships "0000"
                    : (l = await OFe(n,"dongle"), l && parseInt(l) >= parseInt(p) ? o=true : o=false))
    : o = false;
  if (!o) return {...};
  const a = 85;                                             // REPORT ID 85
  if (U6(n.productName)) { /* framed reader CJe — only "V9 PRO 2" and "R9" */ }
  const d = 101;                                            // 0x65
  const h = await dKt({device:n, reportId:a,
        reportData:new Uint8Array([d,1,...new Array(61).fill(0)]), cmdStart:[d]});
  if (h){ switch(h[2]){ case 3: s="charging"; break; case 4: s="full"; break;
                        case 2: s="discharge"; break; case 26: i=true; break }
          return {isSleep:i, batteryLevel:h[1], chargeStatus:s, ...} }
  else return {isSleep:true, ...}
}
function $7t({device:n, getStatusCallback:e}){          // listenChargeStatusChange
  const t = 85, i = [101];
  /* on inputreport with reportId===85 && startsWith([101]):
     level=data[1]; 3 charging / 4 full / 2 discharge / 26 sleep */
}
```

- Wire request: **63 bytes** `[0x65, 0x01, 0x00 ×61]`, sent on report **85**.
- Wire reply: any input report on 85 whose first byte is `0x65`; `level = [1]`,
  `status = [2]`; `26` also means the headset is asleep.
- The framed variant (`CJe`) exists but applies only to products whose name
  contains `V9 PRO 2` or `R9` (`APt` list).
- No battery push was ever captured in any log — the "unsolicited 0x65 push on
  0xFF90" is still an inference.

### 4.3 Firmware (feature report 170)

```javascript
async function OFe(n, e="dongle"){
  const i = new Uint8Array(63).fill(0); i[0] = 1; i[1] = e==="dongle" ? 1 : 0;
  try { await n.sendFeatureReport(170, i) } catch(l){ return null }
  await new Promise(l=>setTimeout(l,300));
  const s = await n.receiveFeatureReport(170).catch(l=>{ /* logs */ });
  return s ? uKt(s) : null
}
function uKt(n){ const e = new Uint8Array(n.buffer);
  return V5(e,[170,1]) ? `${e[2]}${e[3]}${e[4]}${e[5]}` : null }
```

Reply shape `[170, 1, d, d, d, d]` → four ASCII digits. The earlier native port
tolerated both `[170,1,…]` and `[1,…]` layouts; the recovered Rust parser accepts
either `[170, 1, d,d,d,d]` or `[1, d,d,d,d]`.

### 4.4 Notify payload table (from the same bundle module)

`parseNotifyPayloadFromReport` requires `cmdType == [0,4]`, then `cmd` selects the
payload parser. Recovered table (transcript L394):

```javascript
const Il = Object.freeze({
  EqMode:{cmd:1}, VoiceBroadcast:{cmd:2}, ControlDetail:{cmd:3},
  LightGroupsOn:{cmd:5}, SmartLight:{cmd:6}, ControlMode:{cmd:7},
  InitMsg:{cmd:8}, ScreenState:{cmd:9}, ThemeSwitch:{cmd:10}, LyricState:{cmd:11},
  ColorMap:{cmd:14}, ScreenTime:{cmd:15}, ScreenTextState:{cmd:29},
  ExitLyricsDisplay:{cmd:32}, CustomImageName:{cmd:38}, LyricSettings:{cmd:39},
  CustomTextTheme:{cmd:40}, BatterySleep:{cmd:34}});
```

Most of those are keyboard/screen features of other MCHOSE products that share
the framing module; for the V9 Pro only `EqMode` (1) and `BatterySleep` (34) are
plausible.

---

## 5. Evidence ledger: what was actually verified on hardware

Legend: **HW** = observed on the user's own unit (pasted logs / successful
operations), **BUNDLE** = recovered verbatim from the vendor's production JS,
**NATIVE** = static REA evidence on the shipped binaries, **INFER** = the earlier
agent's reasoning, not directly observed.

| Claim | Status | Evidence |
|---|---|---|
| VID/PID/serial/product/manufacturer, `bcdDevice 0x0004` | **HW** | browser log L1143; native app log `… serial 0123456789AB \| bcdDevice 0x0004` (rs transcript L1052) |
| Six collections on one HIDDevice; per-collection report IDs 65/85/170/7/237/255/6 | **HW** | Chrome report lists pasted at web L763/L876/L1027; classification log L1143 |
| WebHID can enumerate, open, read descriptor, receive input reports | **HW** | `IN report 6: 08 00` etc. (L1225, L1548) |
| Named button events: `report 6` → Play/Pause (usage `0xCD`), press = non-zero, release = next zero report | **HW** | L1225/L1548 and native app `Solto: Play/Pause` + `← report 6: 00 00` (rs L1352) |
| Output-report **writes** are accepted by Chrome (no exception) on 7/65/85/237/255 | **HW** | probe log L1235 (`sendError: null`) |
| The device **never answers** framed cmd 12 or raw 0x65 on any of 7/65/85/237/255, with the listener proven alive | **HW** | L1235 (`replies 0`, "device stayed silent for 10 combinations") |
| Native `HidD_SetOutputReport(65, frame cmd 12)` → **Win32 error 31** (device STALLs) | **HW** | rs L1052 22:26:04; L1352 23:28 |
| Native `HidD_SetOutputReport(85, battery)` → **Win32 error 1** INVALID_FUNCTION | **HW** | rs L1052 22:26:16 |
| Native hidapi `WriteFile` output report accepted, no reply | **HW** | rs L1249 23:17:07–23:17:12 |
| Native `HidD_SetFeature(170)` → **Win32 error 1**; Chrome `receiveFeatureReport(170)` refused ("Failed to receive the feature report.") | **HW** | rs L1052 22:26:28; web L1143 20:03:05 |
| Native `HidD_GetIndexedString(50)` returns **`CMA196`** | **HW** | rs app log `string[50]=CMA196` (L1052, L1249) |
| WebUSB is blocked for this device: `claimInterface` → "protected class" (HID class is protected in Chrome) | **HW** | web L763 18:36:47 |
| Frame layout, cmdType table, checksum formula, `reportDataLen=63`, `cmdTypeOffset=4`, reply matcher, 3 s timeout | **BUNDLE** | `purify.es-CSYhaQJg.js` dump (`framing2.txt`, vendor snippets) + JS tests |
| `EQ_HID_CMD` table, mode names, gain codec (cmd 18, int16 BE 0.1 dB, ±12 clamp, ≤54-byte names) | **BUNDLE** | L394 snippet |
| Battery request `[0x65,1,0×61]` on report 85, reply `[0x65, level, status]`, status map 2/3/4/26 | **BUNDLE** | `q7t`/`$7t` snippets |
| Firmware feature 170 request/reply shape, 300 ms wait | **BUNDLE** | `OFe`/`uKt` snippets |
| `V9pro2ReportId = 85` is the report ID used for EQ **and** battery sends | **BUNDLE** | L394 snippet |
| OEM app's APO for this product has **no** HID write path — only `HidD_GetIndexedString` + `lstrcmpW` in `CHidControl::IsAuthentication` | **NATIVE** | REA dossier, `evidence/rea-native-evidence.md` (evidence id `ev_cde74242…`, DLL sha256 `3bd03e32…`) |
| Headset APOs (`V9`, `V9_PRO`, `S9_PRO`, `S9_PRO_14429`, `G9_PRO`) do not import `HidD_SetOutputReport`/`HidD_GetInputReport`; mouse/keyboard APOs do | **NATIVE** | shell scans, `evidence/bash-web.txt` (web L1323, L1333) |
| The vendor frame header `AA 01 00 01 00 01` occurs **zero** times in every native binary of the install (`MCHOSE HUB.exe`, all APO DLLs, drivers) | **NATIVE** | Python byte-scan over the install dir, web L1433 |
| V9 Pro APO contains `CEQ15BandLib_Playback`/`CEQ15BandLib_Recording`, `CPlaybackEFXLib`, `CPlaybackMFXLib` (i.e. an EQ DSP) | **NATIVE** | string dump, web L1337 |
| X9 APO's `HidD_SetOutputReport` caller sits behind `CAuthenticationHandler` ("Init Authentication failed."), i.e. HID writes there are an **authentication handshake**, not settings | **INFER** (strong) | string dump L1462/L1472 + one REA call whose full dossier was not saved; the agent's synthesis is in `README.md` §"Native-side evidence" |
| EQ is host-side DSP (APO) rather than a headset setting | **INFER** (strong) | combination of the two NATIVE rows above + total command silence |
| The 0xFF90 collection pushes battery on report 85 | **INFER** | never observed; inferred from `listenChargeStatusChange` + the collection's declared input 85 |
| `GET_EQ_STATE` (4), `GET_BATTERY_SLEEP` (11), `GET_PROMPT_VOLUME` (13), `GET_AUTO_SHUTDOWN` (14), `SET_AUTO_SHUTDOWN` (26), `FACTORY_RESET` (27), `SET_PROMPT_VOLUME` (33) payload/reply shapes | **INFER** | opcodes are BUNDLE, UI call sites were not recovered |
| Volume **wheel** is on `0x000C` input report 237 | **INFER** | the user said there is a wheel, not a button (web L1588) but no wheel report was ever captured |

The browser probes and native runs are the two independent lines of evidence for
"silent commands", which is why the earlier project treated it as settled for the
browser and as *re-opened* for native (see §7).

---

## 6. Native evidence on the vendor software (REA)

### 6.1 `CmApoIce.dll` for `MCHOSE V9 PRO PID14429`

Path: `libs\S9_PRO_14429\Apo\02_APO\CmApoIce.dll`
(sha256 `3bd03e3275b88c6e6f87fbc37ca02791650d80a793ec8a169fa67ffd1a51aa04`).
The only HID code the session found in it is `CHidControl::IsAuthentication`
(`FUN_180291fa0`, 739 bytes, body `0x180291fa0–0x180292282`), decompiled with
Ghidra 12.1.4; procedure/string/import searches did not turn up any other HID
path. Verbatim from the dossier saved by the session:

```c
bool FUN_180291fa0(longlong param_1, short param_2, short param_3,
                   undefined4 param_4, LPCWSTR param_5)
{
  ...
  HidD_GetHidGuid(&local_140);
  DeviceInfoSet = SetupDiGetClassDevsW(&local_140, 0, 0, 0x12);
  ...
  BVar3 = SetupDiEnumDeviceInterfaces(DeviceInfoSet, NULL, &local_140, MemberIndex, &local_130);
  ...
  pvVar5 = CreateFileW(DeviceInterfaceDetailData->DevicePath, 0xc0000000, 3, NULL, 3, 0x40000000, NULL);
  ...
  cVar2 = HidD_GetAttributes(pvVar5);
  if (((cVar2 != '\0') && (local_150._4_2_ == param_2)) && (local_150._6_2_ == param_3)) break;  // VID/PID
  ...
  cVar2 = HidD_GetIndexedString(*(undefined8 *)(param_1 + 8), local_164, &local_108, 0x40);
  ...
  iVar4 = lstrcmpW((LPCWSTR)&local_c8, local_158);
  bVar6 = iVar4 == 0;
  if (iVar4 != 0) {
    thunk_FUN_1802ac620(L"CHidControl::IsAuthentication[Error] String50 compare false. "
                        L"Device: %s, Expected: %s, ", &local_c8, pWVar1);
  }
  ...
  CancelIo(...); CloseHandle(...); return bVar6;
}
```

Callees (complete list from the dossier): `CreateFileW`, `CancelIo`, `lstrcmpW`,
`EnterCriticalSection`/`LeaveCriticalSection`, `CloseHandle`,
`SetupDiGetClassDevsW`/`SetupDiEnumDeviceInterfaces`/`SetupDiGetDeviceInterfaceDetailW`/
`SetupDiDestroyDeviceInfoList`, `HidD_GetAttributes`, `HidD_GetHidGuid`,
`HidD_GetIndexedString`, `_malloc_base`, `__security_check_cookie`, two logging
thunks. **There is no `HidD_SetOutputReport`, `HidD_GetInputReport`,
`HidD_SetFeature`, `WriteFile` or `DeviceIoControl` in this function**, and a
protocol-ish string scan of the DLL (report/battery/firmware/eq/gain/0xAA…) only
returned DSP symbols — no command opcodes (web L1337).

Strings that anchor it (addresses from the dossier):
`0x1804345f0` "CHidControl::IsAuthentication[Error] OpenDevice() failed.",
`0x180434640` "…String50 compare false. Device: %s, Expected: %s, ",
`0x180434718` "…GetStringIndex() failed.".

Interpretation: the APO **authenticates** the device by comparing HID *string
descriptor index 50* (`CMA196` on this unit) and then does its DSP work. It never
talks to the device beyond that.

### 6.2 Which binaries write HID reports at all

Byte-scan over the whole install (web L1323/L1333):

```
./CM002X1/02_APO/CmApoIce.dll   HidD_GetInputReport HidD_GetPreparsedData HidD_SetOutputReport ReadFile WriteFile
./G9_WIRED/02_APO/CmApoIce.dll  HidD_GetInputReport HidD_GetPreparsedData HidD_SetOutputReport ReadFile WriteFile
./K20/…  ./K20_GT/…  ./K20_PRO/…  ./RzlfVjI=/…  ./VT008/…  ./VT009/…  ./X9/…
                                HidD_GetInputReport HidD_GetPreparsedData HidD_SetOutputReport ReadFile WriteFile
./G9_PRO/02_APO/CmApoIce.dll    ReadFile WriteFile          ← headset family: NO HID write imports
./S9_PRO/02_APO/CmApoIce.dll    ReadFile WriteFile
./S9_PRO_14429/Apo/02_APO/CmApoIce.dll  ReadFile WriteFile
./V9/02_APO/CmApoIce.dll        ReadFile WriteFile
./V9_PRO/02_APO/CmApoIce.dll    ReadFile WriteFile
```

And the frame signature scan (web L1433):

```
pat = bytes([0xAA,0x01,0x00,0x01,0x00,0x01])
exact=   0 aa01= 3614  .\MCHOSE HUB.exe
exact=   0 aa01=  716  .\libs\VT009\02_APO\CmApoIce.dll
exact=   0 aa01=  693  .\libs\S9_PRO_14429\Apo\02_APO\CmApoIce.dll
exact=   0 aa01=  692  .\libs\X9\02_APO\CmApoIce.dll
...
--- done ---          (no file contains the exact 6-byte frame header)
```

Also relevant: the V9 Pro APO's string pool contains
`CEQ15BandLib_Playback`, `CEQ15BandLib_Recording`, `CPlaybackEFXLib`,
`CRecordingEFXLib`, `CPlaybackMFXLib`, `CRecordingMFXLib` (web L1337) — a 15-band
EQ DSP, which is what actually applies the EQ the UI shows.

### 6.3 Where the *product lists* live (and what they really gate)

Two different lists were conflated in the earlier write-up; the recovered bundle
shows both distinctly:

```javascript
// (a) battery framing list — only these two use the framed reader
APt = [{productKey:"MCHOSE V9 Pro 2", name:"V9 PRO 2", versionDongle:"0000", versionHeadset:"0000"},
       {productKey:"MCHOSE R9",       name:"R9",       versionDongle:"0000", versionHeadset:"0000"}]
function U6(n){ const e=(n?.toUpperCase())??""; return APt.some(t=>e.includes(t.name.toUpperCase())) }

// (b) audio EQ profile list — the host-side DSP profiles
AUDIO_EQ_PROFILE_PRODUCT_KEYS = Object.freeze([
  "MCHOSE V9 PRO 2$$$14391$$$24593",
  "MCHOSE R9$$$14391$$$24595"])
function isAudioEqProfileProductKey(S){ ... }
```

The V9 Pro's key `MCHOSE V9 Pro--14429--10525` / `MCHOSE V9 PRO$$$10525$$$14429`
appears in **neither** list. What that means precisely:

- the **framed battery protocol** is not used for the V9 Pro (it uses the raw
  0x65 path) — recovered code, no ambiguity;
- the **audio EQ profile** (host DSP, i.e. APO-side presets) is implemented for
  V9 PRO 2 and R9 only; for the V9 Pro the same UI features are served by the
  C-Media APO (`audio-sdk:*` IPC) — consistent with §6.1/§6.2;
- it does **not** prove that the HID EQ opcodes are product-gated: the recovered
  `getCurrentEQMode`/`setEQMode`/`setPEQAllGain` functions take any `device` and
  reference `reportId` (85) without a product check.

---

## 7. What the Rust companion implemented (and where it stops)

Single binary, one crate, `hidapi` + `egui/eframe` + `nusb` (raw-USB experiment),
UI in pt-BR. Modules:

- `src/protocol.rs` — device-agnostic `0xAA` codec (`build_report`, `checksum`,
  `payload_of`, `length_of`, `verify_frame`, `matches_reply`, `decode_mode_name`,
  `encode_name`) + unit tests with the same vectors as the JS suite.
- `src/transport.rs` — `hidapi` wrapper that centralises the report-ID
  convention (prefix on write, strip on read) and enumerates by VID/PID.
- `src/device/v9pro.rs` — constants (`PAGE_COMMAND 0xFF82`, `REPORT_COMMAND 65`,
  `PAGE_NOTIFY 0xFF90`, `REPORT_NOTIFY 85`, `PAGE_FIRMWARE 0xFF22`,
  `REPORT_FIRMWARE_FEATURE 170`, `STOCK_REPORT_ID 85`, `BATTERY_RAW_CMD 0x65`),
  builders/parsers for cmd 12/1/18/24/27, battery, firmware.
- `src/models.rs` — `Battery`, `ChargeStatus`, `BatterySource`, `FirmwareVersion`,
  `EqGains` (`encode_peq_payload` = cmd 18 int16 BE, clamp ±12), `ButtonEvent`.
- `src/worker.rs` — I/O thread: read loop, pending-request matcher, passive
  battery sniffer, probe matrix (hidapi / HidD / raw USB), multi-handle fan-out.
- `src/win_raw.rs` — Windows FFI to `HidD_SetOutputReport`/`HidD_GetInputReport`
  (a *different* code path from hidapi's `WriteFile`), plus indexed-string read.
- `src/raw_usb.rs` — `nusb` control-transfer experiment (HID SET_REPORT/GET_REPORT).
- `src/ui/*` — Dispositivo / Status / EQ / Console; EQ has 3 modes, 10 sliders
  (±12 dB), cloud presets, "Aplicar EQ", and an explicit **"⚠ sem resposta do
  fone"** state (it never pretends success).
- `docs/ANALISE-SONDA.md` — the revalidation log + the de-obfuscated vendor
  recipe (battery `q7t`/`$7t`, firmware `OFe`, report 85, all-handles strategy).

**Known limitations of the v1 (as documented by the project itself):**

1. Host-side EQ (APO / PipeWire filter-chain / Audio Unit) is **out of scope**:
   the app can only *send* HID EQ commands and report that nothing answered.
2. The native re-probe is **unvalidated**: the multi-handle "battery on all
   handles (vendor r85)" cell and the "feature 170 across all handles" sweep were
   added in the last 20 minutes of the session and never executed on hardware
   (the session ends right after; the project memory also says "o usuário ainda
   não rodou no hardware").
3. Battery relies either on a poll that never answered or on a push never
   observed → in practice the card stays "Desconhecido".
4. Firmware 170 cannot even be *sent* on the tested handle (error 1), so the
   dialog only shows `bcdDevice 0x0004` as a proxy.
5. The button/wheel events use a simplified decoder (hard-coded report 6 =
   Play/Pause; report 237 = wheel candidate) instead of parsing the full HID
   report descriptor.
6. `assets/presets.json` is **wrong** — see §8 item 7.

---

## 8. Contradictions and open questions (ranked by impact)

### 8.1 Report ID: descriptor says 65, vendor code says 85; the two descriptor readers also disagree about `0xFF90` having an output report

Three observations that cannot all be right:

| Observer | `0xFF82` | `0xFF90` |
|---|---|---|
| Chrome `HIDDevice.collections` (HW) | input [65], output [65] | input [85], **no output** |
| Windows `HidP_GetCaps` per collection (HW) | in 64, out 64 | in 64, **out 64** |
| Vendor bundle (BUNDLE) | — | sends EQ + battery on **report 85** |

- Browser probe: sending raw `[0x65,1,…]` on **85** produced no reply, and
  Chrome logged no send error.
- Native: `HidD_SetOutputReport(85, …)` on the command handle → error 1;
  `HidD_SetOutputReport(65, …)` → error 31 (STALL).

**Unresolved.** If Windows is right that `0xFF90` has a 64-byte output report,
then report 85 *is* a legal output report on that handle and the vendor path is
physically possible — the failed native attempts used the wrong handle
(`HidD_SetOutputReport(85, …)` was issued on the *command* handle) and the
never-executed multi-handle sweep was exactly the right test. If Chrome is right,
the vendor's `sendReport(85, …)` can only be a no-op/failed promise.

Either way the conclusion for a rewrite is the same: **test (handle × report ID)
pairs, not report IDs alone**, and re-derive the true report-ID map by parsing the
raw HID report descriptor
(`hid_get_report_descriptor` / `IOCTL_HID_GET_REPORT_DESCRIPTOR`) instead of
trusting either API's summary.

*Cheap first experiment:* re-run the native enumerator and dump the raw report
descriptor from the `Col05`/`Col06` interface paths, then parse output report IDs
from the bytes. *Definitive experiment:* trace M HUB while it displays battery
(ETW/API Monitor/Frida on `WriteFile`/`HidD_SetOutputReport`), or open M HUB's
renderer devtools and log `navigator.hid` traffic.

### 8.2 "The V9 Pro is not in the EQ product list" is over-read

The earlier `README.md`/`ANALISE-V9-PRO.md` concluded the V9 Pro is unsupported
because the vendor's EQ list contains only `MCHOSE V9 PRO 2` and `MCHOSE R9`.
The recovered code shows that list is `AUDIO_EQ_PROFILE_PRODUCT_KEYS` (host-side
audio profile) and `APt` (battery framing) — **not** the HID EQ command gating.
The correct statement is: *the vendor applies EQ to this SKU through the C-Media
APO, not through a device-side profile*, while the HID EQ opcodes remain present
and product-agnostic in the bundle. Whether the firmware on this unit implements
them is still open (it never answered).

### 8.3 "Battery comes from IPC" vs "battery comes from HID report 85"

The web project's synthesis says the battery shown by M HUB comes from Electron
IPC (`onBatteryInfo`), while also noting that the IPC string is absent from the
main-process bytecode ("legado/morto"). The bundle says the renderer's own
`q7t` reads battery over HID report 85. Both cannot be true on this machine
unless the IPC path is dead and the HID path works — which contradicts the
10/10-silent probes. **Unresolved**; this is the single most valuable thing to
settle because it decides whether a native rewrite can ever show battery.

Sub-question: does `0xFF90` ever *push* `[0x65, level, status]`? No log in either
session contains such a push, even though the listener was armed across several
"Listen only" windows (10 s each) and every probe cell.

### 8.4 Firmware feature 170

Chrome: `sendFeatureReport(170, …)` resolves, `receiveFeatureReport(170)` throws
("Failed to receive the feature report"). Native: `HidD_SetFeature(170, …)` fails
with error 1 on the tested handle; `hidapi` also reported
`HidD_SetFeature: (0x00000001) Função incorreta`. So on this unit the feature
report is **not usable** through the APIs tried. Unknown whether another handle
or `IOCTL_HID_SET_FEATURE` with the right collection would work. Note the vendor
only calls 170 when `enableBatteryVersion !== "0000"`; this unit ships `"0000"`,
so the stock app may never exercise it either.

### 8.5 Error 31 vs "accepted" for the same frame

- hidapi `WriteFile` to the command handle → returns success, no reply.
- `HidD_SetOutputReport` on the same report ID → error 31 (the control pipe
  STALLs).

These are different transports: interrupt OUT vs control SET_REPORT. The STALL is
*active rejection by the device* for the control-pipe variant, while the interrupt
OUT accepts bytes and ignores them. Any reimplementation must test both paths per
handle and treat "accepted" as meaningless until a reply arrives.

### 8.6 Native multi-handle behaviour is inconsistent across runs

At 22:26 hidapi battery on report 65 was "sent, no reply"; at 23:29 the same code
reported `WriteFile: (0x00000001) Função incorreta`. The difference is which
handle was tried first (the app was being reworked in between). So per-handle
capabilities differ — the 0xFF82 handle accepts output, the 0xFF90 handle does
not. Any matrix must record *(handle, report ID)* pairs, not just report IDs.

### 8.7 The Rust presets do not match the vendor cloud data

`assets/presets.json` differs from the verbatim CDN JSON for **7** presets and
contains **4** presets that do not exist for this device (`CS 3`, `CF 2`,
`Delta Force 2`, `Delta Force 3`). The cloud file also contains gains outside the
documented ±12 dB range (e.g. `-14`), which is why the send path clamps.
**Use `prior-art/read-artifacts/web/.rea-work/cdn/v9pro/data.json`**, not the
Rust asset. See `prior-art/verification/presets-vs-cloud.txt`.

### 8.8 Smaller contradictions / open items

- **`SET_PEQ_ALL` (16) vs 18:** the table declares 16 but the only gain sender
  uses 18. 16 may be a different (all-bands?) command; unused in the bundle.
- **Checksum "enabled" never seen on the wire**: `check` defaults to false
  everywhere. A firmware that requires the checksum flag was a probe variant
  only, never a vendor behaviour.
- **`length` semantics**: the length byte counts payload **+1** (the cmd byte).
  Two independent decoders (`protocol.js`, vendor `tp`) agree; a third-party
  implementation that treats it as payload-only will be off by one everywhere.
- **`GET_REPORT` on 170 and report-ID duplication**: browser `receiveFeatureReport`
  may or may not include the ID byte; both implementations tolerate either.
- **The wheel**: user-confirmed it is a wheel (web L1588) but no wheel HID report
  was captured; report 237 on `0x000C` remains a hypothesis.
- **Memory vs logs:** `~/.claude/.../memory/projeto-mchose-v9pro-rs.md` (written
  2026-10-14T20:54) says the native probe "ainda não rodou no hardware"; the
  transcript shows three hardware rounds after that (22:26, 23:17, 23:28). Treat
  the memory as stale.
- **`0x000C` output report 255** exists in the descriptor but nothing in the
  recovered code uses it.

---

## 9. What this means for the MCHOSE HUB reimplementation

1. **Copy the codec and the constants, not the conclusions.** Frame layout,
   CmdType values, EQ opcodes, gain encoding, battery/firmware payloads are
   BUNDLE-recovered and test-verified. The "this SKU ignores commands" conclusion
   is HW-observed for the browser, but its *explanation* (host-side EQ) is an
   inference.
2. **Model the transport as (handle × report ID).** Windows opens a separate
   handle per top-level collection; Chrome gets one composite device. All native
   evidence says per-handle behaviour differs, so the app should enumerate
   handles, store `usage_page`, and try *(handle, report)* cells explicitly.
3. **Try both delivery paths** for each cell: interrupt OUT (`WriteFile`/hidapi)
   and control SET_REPORT (`HidD_SetOutputReport`), and record the Win32 error.
   The 31-vs-1 split is diagnostic information, not noise.
4. **Do not build UI on top of unverified HID reads.** Battery/firmware/EQ must
   degrade to "no reply" states; the prior Rust app did this correctly and is a
   good UX reference.
5. **The authoritative preset data is the CDN JSON** recovered in
   `prior-art/read-artifacts/web/.rea-work/cdn/v9pro/data.json` (24 presets,
   10 bands, values −14…14). The Rust asset is corrupt prior art.
6. **Host-side EQ is where the vendor puts the DSP** (C-Media APO with
   `CEQ15BandLib_*`); a faithful reimplementation of "real" EQ needs an APO /
   filter chain, not a HID command.
7. **The definitive experiment is a live trace of M HUB** while it reads battery
   and applies EQ. Nothing static will resolve §8.1/§8.3.
8. Useful collateral for the current project: the recovered
   `prior-art/read-artifacts/web/.rea-work/asar/**` files (MCHOSE HUB
   `package.json`, `out/preload/index.js`) and the vendor-bundle snippets
   (`framing2.txt`, `eq_core.txt`) — these are the exact strings the earlier
   session mined out of the ASAR and the CDN bundles.

---

## 10. Reproduction and evidence index

Everything is reproducible from `prior-art/tools/` (python 3.12, no deps):

| Script | Purpose |
|---|---|
| `recover_prior_art.py` | rebuild both trees (file-history + replay, reconciled) and `recovered-summary.json` |
| `recover_read_artifacts.py` | rebuild `read-artifacts/` (cloud preset JSON, ASAR files, mined snippets) |
| `extract_evidence.py` | dump human turns, shell output, REA results per session |
| `curate_rea_evidence.py` | turn the saved REA dossiers into `evidence/rea-native-evidence.md` |
| `extract_vendor_snippets.py` | pull the vendor-bundle snippets out of shell output into `evidence/vendor-bundle-snippets.md` |
| `forensics_mutations.py` | per-file mutation timeline (tool call, timestamp, tool_result status) |
| `compare_presets.py` | vendor cloud presets vs Rust asset (exits 1 on mismatch) |
| `show_summary.py` | human-readable view of `recovered-summary.json` |

Evidence files:

- `evidence/human-turns-web.md`, `evidence/human-turns-rs.md` — every human turn
  with transcript line number and timestamp (the hardware logs quoted above are
  the `pasted_content` blocks there).
- `evidence/bash-web.txt`, `evidence/bash-rs.txt` — every shell call and its
  output (vendor bundle mining, PE import scans, frame-signature scan).
- `evidence/rea-native-evidence.md` — curated REA dossiers with evidence IDs and
  artifact digests (1.9 MB; the `CmApoIce.dll` dossier is at the top).
- `evidence/rea-results-web.md`, `evidence/rea-results-rs.md` — raw REA results as
  they appear in the transcript (some are truncated by the client, noted there).
- `evidence/vendor-bundle-snippets.md` — the vendor JS snippets with source line
  citations.
- `read-artifacts/web/.rea-work/cdn/v9pro/data.json` — **verbatim** vendor cloud
  preset file for `MCHOSE V9 Pro--14429--10525`.
- `verification/` — `VERIFICATION.md`, `cargo-check.log`, `cargo-test.log`,
  `presets-vs-cloud.txt`.

Session references:

| Project | Session id | Span (UTC) | Records |
|---|---|---|---|
| mchose-web | `d10fc331-bad8-4866-a434-bd47940b5e19` | 2026-10-13T17:20:10Z → 2026-10-13T22:53:45Z | 1677 |
| mchose-v9pro-rs | `df0e544e-2f98-407d-a2f2-26eb5af84be2` | 2026-10-14T19:49:09Z → 2026-10-15T00:28:58Z | 1547 |

Transcripts: `C:\Users\João\.claude\projects\C--Users-Jo-o-Documents-mchose-web\`.
