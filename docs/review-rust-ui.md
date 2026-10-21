# Review — Rust UI pages vs. recovered renderer specs

Scope: `mchose-hub-rs/src/ui/pages/{effects,routine,mic,eq,surround}.rs` and
`src/model/audio.rs`, checked against `mchose-hub-re/docs/findings-renderer.md` §7.1–7.7 and the
recovered vendor bundles (`renderer/pretty/prod/index-BkAOL4Ty.pretty.js`,
`index-Cbzg32lo.pretty.js`, `index-C2a…/index-DWc…`, `eq.json` on disk).
Read-only review; no Rust files were modified.

Legend: **[P1]** wrong value → user-visible wrong behaviour; **[P2]** missing/extra control or
layout; **[P3]** asset/data problem; **[P4]** cosmetic/provenance.

---

## P1 — ranges and defaults

| # | File:line | Current | Corrected (verified cmedia / V9 PRO) |
|---|---|---|---|
| 1 | `effects.rs:125` | VirtualBass level slider `0..=100` | **`0..=10`, step 1**, default **5** (`setVIRTUALBASS_Level`) |
| 2 | `effects.rs:134` | Bass cutoff `20..=20_000` Hz | **`100..=400` Hz, step 50**, default **200**; slider marks 100/200/300/400. The 20–20000 range only exists on the THX branch (where the cutoff row is not rendered) |
| 3 | `effects.rs:139` | `cutoff = ((cutoff + 25)/50*50).clamp(20, 20_000)` | `clamp(100, 400)` |
| 4 | `effects.rs:148` | AdaptiveVolume level `0..=10` | **`0..=100`, step 1** (both runtimes; only end-labels differ: cmedia none, THX `naturalDynamics`→`stableVolume`). `mode` 0/1 is correct (cmedia-only control) |
| 5 | `effects.rs:115,126,150,170` | level rows suffixed `" dB"` | Original **level rows show a bare number**; only the voice-clarity **noise** row carries `unitDb` and only the bass **cutoff** row carries `unitHz` (`speakerSettings.unitDb`/`unitHz`) |
| 6 | `effects.rs:205-240` | reset disables the block and restores defaults | Vendor reset buttons behave differently: **treble reset only rewrites level** (never disables); **bass and volume-balance reset buttons are rendered only on THX devices**; clarity reset rewrites level+noise. Turning the switch off on reset is a deviation |
| 7 | `model/audio.rs:168` | `VolumeState::default().mic = 1.0` | Vendor factory reset writes **mic scalar `0.92`** (`setVolumeScalarControl(-1, 0.92, "mic")` in the Other-settings reset sequence) |
| 8 | `model/audio.rs:18-19` + `eq.rs:107,124` | `EQ_GAIN_MIN/MAX = -12..12` used as slider range | Shipped presets contain **-14..+14** (music 高音 `+14`, 重金属 `+13`, 饶舌曲 `-13`; game 无畏契约2 `-14`). A ±12 editor cannot represent/applies-clamped vendor presets. Widen the editor range (e.g. ±15) or keep raw values when applying a preset and clamp only on user edit |

## P2 — routine page (legacy landing page) composition

Verified original `routineLegacy` (V9 PRO): **hero product image** + footer with **three columns**:
1. `volume` card · (`volumeBroad` only when `ifVolumeBroadcast`, i.e. V9 TURBO family) · `mic` card;
2. `surroundAmplification` · `lowFrequency`;
3. `smartVolume` · `vocalsClarity`.

| # | File:line | Finding |
|---|---|---|
| 9 | `routine.rs:12-31` | No **hero device image** and the layout is a vertical stack; original is hero + 3-column footer grid |
| 10 | `routine.rs:33-71` | Volume card is OK (volume + mute + Windows-sound button), but the original **mic card sits in the same row/column** as volume, not on its own row |
| 11 | `routine.rs:73-148` | **Missing mic volume + mic mute** in the routine mic card. Original `mic` component calls `setVolumeScalarControl(ch,"mic")`, `setMuteControl(v,"mic")`, plus a mic-enable switch, the "Windows声音设备" button and a mic-disabled recovery guide (`open_mic_guide.svg`, `routine.step/step1/step2/step3`) |
| 12 | `routine.rs:172-204` | **Missing `surroundAmplification` toggle** (`get/setEnable_Channel_COPY_LFX`; labels `routine.SurroundAmplification` + desc). It is part of the legacy landing page (column 2) and has no control anywhere in the port |
| 13 | `routine.rs:172-204` | `lowFrequency` / `smartVolume` / `vocalsClarity` appear only as **on/off toggles** in the "effects overview"; the original landing page shows the **value cards with sliders** in columns 2–3 (the effects page duplicates them, so this is a layout-parity gap rather than missing functionality) |
| 14 | `routine.rs:124-137` | Extra **mic-listen** row on the routine page, permanently disabled. The legacy routine page has no mic-listen control (mic listen belongs to the newer `audio_mic`/routine layouts). Harmless but a deviation |
| 15 | — | **Voice prompt volume 1..16** *(requested by Lead)*: do **not** add for V9 PRO. Verified: the `routine.beepTitle` slider lives in the **new-layout** `routineCommonParams` and is driven by HidEQ cmds 13/33; it is not part of `routineLegacy` |
| 16 | — | **Voice broadcast toggle** *(requested by Lead)*: `volumeBroad` is rendered only when `ifVolumeBroadcast` is true (V9 TURBO+ family), so its absence is correct for V9 PRO; add only behind a capability flag |

## P3 — preset asset (`assets/presets.json`) vs device `eq.json`

Counts match (13 game + 11 music). Vector comparison against the connected device's actual
`files/config/eq.json`:

- **MUSIC: 11/11 identical.** ✅
- **GAME: 8/13 identical; 5 differ:**

| preset | asset vector | eq.json (ground truth) |
|---|---|---|
| VALORANT 1 | `[-10,-5,5,-9,-3,0,1,2,2,0]` | `[-6,-5,0,-6,0,0,6,-3,2,0]` |
| VALORANT 2 | `[-10,-6,6,-14,-4,-1,1,0,1,0]` | `[-2,1,4,6,-2,1,3,2,-3,1]` |
| VALORANT 3 | `[-6,-5,0,-6,0,0,3,-3,1,0]` | `[-10,-5,10,-2,3,2,0,2,3,0]` |
| CF 1 | `[-4,1,9,1,-3,-1,4,5,3,1]` | `[5,-1,3,2,-1,-1,1,0,0,1]` |
| Delta Force 1 | `[-3,-3,9,1,2,4,-2,1,2,0]` | `[-3,-3,11,4,8,4,4,5,5,0]` |

Recommendation: `eq.json` is the persisted store of the exact connected device → treat it as the
ground truth for V9 PRO and replace those five vectors (or tag the asset as a different firmware
generation).

- **Game ordering differs** from the vendor list: the asset interleaves CS between VALORANT entries
  (`default, v1, v2, cs1, v3, cs2, …`), while `eq.json` groups by family
  (`默认, 无畏契约1..3, CS1, CS2, Apex1/2, 绝地求生1..3, CF1, 三角洲1`).
- **`names.zh` strings are mojibake** in the asset (double-encoded UTF-8 read as Latin-1):
  `"æ¸¸æˆ"` (游戏), `"é»˜è®¤"` (默认), `"æ— ç•å¥‘çº¦"` (无畏契约). Displaying `zh` will show garbage —
  re-encode the file (Node: read as utf8 → `Buffer.from(s,'latin1').toString('utf8')`) or replace the
  CJK strings.

## P4 — verified-good / provenance notes

- `surround.rs` matches the recovered Xear page well: enable switch, **music/game vs movie** mode,
  **small/medium/large** room, 8-channel speaker test (`L/R-Front, Center, Subwoofer, L/R-Surround,
  L/R-Back`) + stop. Only missing the help popover text (`surround.popover`, `surround.popoverGame`:
  "7.1 not for FPS").
- `mic.rs` is an *additional* page (the V9 PRO original has no mic page; its mic controls live on the
  routine card). AI-NR/NR/magic-voice map to real preload methods, so this is a reasonable surfacing;
  just note that the **legacy** mic card is mute+volume+magic-voice, not the AI-NR set.
- `EQ_FREQUENCIES_HZ = 31..16000` (model/audio.rs:14) is **inferred**, not evidenced: the vendor
  editor only labels three captions (低音/中音/高音) — no band-centre table was recovered.
- `eq.rs` clamp-on-apply (item 8) and `effects.rs` ranges (items 1–4) are the only user-visible
  numeric errors found; everything else in the reviewed pages is consistent with the recovered
  specs.

---

### Reusable scripts added while reviewing
- `mchose-hub-re/renderer/scripts/diff-presets.js` / `diff-presets2.js` — preset asset ↔ `eq.json` diff.
- `mchose-hub-re/renderer/scripts/print-template.js` — print template/logic lines of a beautified chunk.
- `mchose-hub-re/renderer/scripts/check-doc.js` — UTF-8/mojibake validation for docs.
