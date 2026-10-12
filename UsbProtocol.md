# SoundBlasterX G6 — USB HID Protocol Reference v4

*Reverse-engineered from USB captures, February 2026*

---

## Table of Contents

1. [Transport Layer](#transport-layer)
2. [Frame Structure](#frame-structure)
3. [Device Initialization](#device-initialization)
4. [Command Reference](#command-reference)
   - [0x02 — DeviceAck](#0x02--deviceack)
   - [0x05 — DeviceIdentify](#0x05--deviceidentify)
   - [0x06 — Ping](#0x06--ping)
   - [0x07 — GetFirmwareString](#0x07--getfirmwarestring)
   - [0x10 — GetSerial](#0x10--getserial)
   - [0x11 — StatusRequest / StatusResponse](#0x11--statusrequest--statusresponse)
   - [0x12 — WriteSingleFeature](#0x12--writesinglefeature)
   - [0x15 — BulkRangeDump](#0x15--bulkrangedump)
   - [0x20 — GetHardwareId](#0x20--gethardwareid)
   - [0x26 — GlobalProfile (SBX/Scout/EQ)](#0x26--globalprofile)
   - [0x2c — OutputSelect](#0x2c--outputselect)
   - [0x30 — GetDspVersion](#0x30--getdspversion)
   - [0x3a — Capabilities / RGB Lighting](#0x3a--capabilities--rgb-lighting)
   - [0x3c — GainConfig](#0x3c--gainconfig)
   - [0x6c — DacFilter](#0x6c--dacfilter)
   - [0x6e — Notification](#0x6e--notification)
5. [Feature Family 0x96 — DSP Effects](#family-0x96--dsp-effects)
6. [Feature Family 0x95 — Recording Pane](#family-0x95--recording-pane)
7. [How To: Common Operations](#how-to-common-operations)
8. [Open Questions](#open-questions)
9. [Appendix A: Known But Unused](#appendix-a-known-but-unused)
   - [Multi-Feature StatusResponse](#multi-feature-statusresponse)
   - [0x39 — Direct Mode](#0x39--direct-mode)
   - [Family 0x97 — Hardware / Decoder](#family-0x97--hardware--decoder)

---

## Transport Layer

All communication uses USB HID SET_REPORT on **interface 4**.

| Field          | Value  | Notes                         |
|----------------|--------|-------------------------------|
| bmRequestType  | `0x21` | Class, Interface, Host-to-Dev |
| bRequest       | `0x09` | SET_REPORT                    |
| wValue         | `0x200`| Output Report, Report ID 0    |
| wIndex         | `0x04` | Interface 4                   |
| wLength        | `0x40` | 64 bytes, always              |

- **Host → Device:** Control transfer, endpoint `0x00`
- **Device → Host:** Interrupt transfer, endpoint `0x85`
- All packets are **64 bytes**, zero-padded.

### Important: Linux Requires USB Reset

On Linux, the interrupt IN endpoint on interface 4 is not active after enumeration.
The device requires a **USB reset** (`USBDEVFS_RESET`) before it will respond on the interrupt pipe.
This only needs to happen once per USB session (boot/plug-in/resume).
The reset affects the entire device (briefly interrupts audio).

### HID Report ID

When using hidapi, prepend `0x00` (report ID) before the 64-byte payload, making the write buffer 65 bytes.
When using raw USB (pyusb/rusb control transfers), send the 64 bytes directly.

---

## Frame Structure

```
Byte:  0     1      2       3..63
       0x5a  CMD    LEN     PAYLOAD (zero-padded to 64 bytes)
```

- `0x5a` — Magic byte, always present
- `CMD` — Command identifier
- `LEN` — Length of payload (bytes following LEN)
- Payload structure varies per command

---

## Device Initialization

After USB reset, the SBX software runs this sequence (repeated 3-4 times during startup):

```
Phase 1 — Device Identification
  0x05  DeviceIdentify
  0x10  GetSerial
  0x20  GetHardwareId
  0x30  GetDspVersion
  0x39  HwConfig sub=0x02
  0x06  Ping
  0x15  BulkRangeDump
  0x07  GetFirmwareString
  0x3a  Capabilities queries (sub 0x00, 0x05, 0x07, 0x09, 0x0b, 0x0e, 0x10)

Phase 2 — Feature State Readback
  0x11  Read all 0x95 features
  0x11  Read all 0x96 features
  0x26  Global profile query (SBX/Scout/EQ bitmask)
  0x2c  Output mode query
  0x3c  Gain config query
  0x6c  DAC filter query

Phase 3 — Apply Saved Profile
  0x12  Write mic EQ bands (0x95:0x14–0x1b)
  0x12  Write EQ bands (0x96:0x0a–0x14)
  0x12  Write EQ toggle (0x96:0x09)

Phase 4 — RGB Lighting Init
  0x3a  sub=0x06 (RGB enable/disable)
  0x3a  sub=0x04 (LED mode config)
  0x3a  sub=0x0a (LED color data)
```

**Minimal init for a control tool:** USB reset, then immediately start reading/writing features.
The device responds to commands without needing the full handshake.

---

## Command Reference

### 0x02 — DeviceAck

Direction: **Device → Host** (unsolicited, follows writes)

```
5a 02 0a [echoed_cmd] 00 [payload_echo or zeros]
```

Total payload length is always `0x0a` (10 bytes).

---

### 0x05 — DeviceIdentify

Query device presence and capability flags.

```
Send: 5a 05 00
Recv: 5a 05 04 [flags] 00 00 00
```

Observed: flags = `0x1f` = `0b00011111`

**UNKNOWN:** Individual meaning of flag bits.

---

### 0x06 — Ping

Keepalive / connectivity test. Device echoes the packet back.

```
Send: 5a 06 01 01
Recv: 5a 06 01 01
```

---

### 0x07 — GetFirmwareString

Query firmware version as ASCII string.

```
Send: 5a 07 01 [type]
Recv: 5a 07 [len] [ASCII string bytes...]
```

| Type   | Returns                    |
|--------|----------------------------|
| `0x02` | Firmware version string    |

Observed: `"2.1.250903.1324"`

**UNKNOWN:** Other type values.

---

### 0x10 — GetSerial

```
Send: 5a 10 00
Recv: 5a 10 08 [serial_bytes × 8]
```

Observed: `ef 67 74 00 00 00 00 00`

---

### 0x20 — GetHardwareId

```
Send: 5a 20 00
Recv: 5a 20 04 [hw_id] 00 00 00
```

Observed: `0x97`

---

### 0x30 — GetDspVersion

```
Send: 5a 30 00
Recv: 5a 30 04 [version_bytes × 4]
```

Observed: `30 01 10 00`

---

### 0x11 — StatusRequest / StatusResponse

Read the current value of a single feature. Used for all features in families 0x95, 0x96, 0x97.

**Request (Host → Device):**
```
5a 11 03 01 [family] [feature_id]
```

**Response (Device → Host):**
```
5a 11 08 01 00 [family] [feature_id] [f32_value_LE × 4 bytes]
```

The f32 value is IEEE 754 little-endian.

**Unsolicited responses:** The device also sends `0x11` packets unprompted after writes,
especially when toggling SBX or switching outputs (cascading state notifications).

Example — read CrystalizerToggle:
```
Send: 5a 11 03 01 96 07
Recv: 5a 11 08 01 00 96 07 00 00 80 3f    → 1.0 (ON)
```

See [Appendix A: Multi-Feature StatusResponse](#multi-feature-statusresponse) for the bulk variant.

---

### 0x12 — WriteSingleFeature

Write a value to a feature. Used for all features in families 0x95, 0x96, 0x97.

```
Send: 5a 12 07 01 [family] [feature_id] [f32_value_LE × 4 bytes]
```

LEN=`0x07` = 7 payload bytes: `01 [family] [feature_id] [f32 × 4]`. There is **no padding byte** between `feature_id` and the f32. This differs from the StatusResponse, which has an extra `00` at d[4] and places the f32 at d[7..11].

Device responds with:
1. `0x02` ACK (immediate)
2. `0x11 0x08` StatusResponse (unsolicited push, confirms new value)

Example — set CrystalizerToggle to ON:
```
Send: 5a 12 07 01 96 07 00 00 80 3f
Recv: 5a 02 0a 12 00 00 00 00 00 00 00 00 00    ← ACK
Recv: 5a 11 08 01 00 96 07 00 00 80 3f           ← status push
```

**Value encoding:**
- Boolean toggles: `0x00000000` (0.0f) = OFF, `0x3f800000` (1.0f) = ON
- dB values: f32 literal (e.g., `0xc0533333` = -3.3 dB)
- Percentage levels: 0.0–1.0 float (e.g., `0x3f000000` = 0.5)

---

### 0x15 — BulkRangeDump

Returns feature value ranges (max, min, step) for features with configurable limits.

```
Send: 5a 15 01 00
Recv: 5a 15 [len] [unknown_u8] [count_u8] [entries...]
```

Each entry is **14 bytes:** `family(1) id(1) max_f32(4) min_f32(4) step_f32(4)`

**Observed entries:**

| Family | ID   | Feature      | Max     | Min     | Step |
|--------|------|--------------|---------|---------|------|
| 0x96   | 0x0a | EqPreAmp     | +6.0 dB | -6.0 dB | 0.5  |
| 0x96   | 0x0b | Eq31Hz       | +12.0 dB| -12.0 dB| 0.5  |
| 0x96   | 0x17 | SurroundDist | 300.0   | 10.0    | 0.5  |

**Note:** Only 3 entries returned. The EQ band range (0x0b) likely applies to all 10 bands (0x0b–0x14).

---

### 0x26 — GlobalProfile

Controls and queries the three global toggles: SBX Master, Scout Mode, EQ Enable.

#### Read bitmask

```
Send: 5a 26 03 08 ff ff
Recv: 5a 26 0b 08 ff ff [bitmask] 00 00 00 00 00 00 00
```

**Bitmask (confirmed):**

| Bit | Mask   | Feature     | Notes                              |
|-----|--------|-------------|------------------------------------|
| 0   | `0x01` | SBX Master  | Mutually exclusive with Scout Mode |
| 1   | `0x02` | Scout Mode  | Mutually exclusive with SBX Master |
| 2   | `0x04` | EQ Enable   | Independent, survives both         |

**Observed values:**
- `0x05` = SBX ON + EQ ON (normal state)
- `0x04` = SBX OFF + EQ ON
- `0x06` = Scout ON + EQ ON (SBX auto-disabled)
- `0x01` = SBX ON + EQ OFF

#### Write toggles

```
SBX Master:  5a 26 05 07 01 00 [00|01] 00
Scout Mode:  5a 26 05 07 02 00 [00|01] 00
```

EQ Enable is written via feature write instead: `5a 12 07 01 96 09 [f32]`

**Write format breakdown:**
```
5a 26 05 07 [feature_id] 00 [state] 00
             ^^                ^^
             0x01=SBX           0x00=OFF
             0x02=Scout         0x01=ON
```

Device responds with ACK (`0x02`) that echoes the payload.
Toggling SBX/Scout triggers cascading `0x11` status pushes for affected DSP features.

---

### 0x2c — OutputSelect

Controls headphone/speaker output mode. Uses the direction-flag pattern (byte 3):
`0x00` = write, `0x01` = read response, `0x02` = enumerate.

#### Read current output

```
Send: 5a 2c 01 01
Recv: 5a 2c 05 01 [mode] 00 00 00
```

#### Enumerate available outputs

```
Send: 5a 2c 01 02
Recv: 5a 2c 0a 02 82 02 00 00 00 04 00 00 00
```

#### Write output selection

```
Send: 5a 2c 05 00 [mode] 00 00 00
```

**Output values:**

| Value  | Output     |
|--------|------------|
| `0x02` | Speakers   |
| `0x04` | Headphones |

---

### 0x3a — Capabilities / RGB Lighting

Dual-purpose command family. Lower sub-commands query device capabilities, higher ones control RGB lighting.

#### Capability Queries (read-only, used during init)

```
Send: 5a 3a [len] [sub] [data...]
Recv: 5a 3a [len] [sub] [data...]
```

| Sub    | Request data     | Response data              | Interpretation               |
|--------|------------------|----------------------------|------------------------------|
| `0x00` | (none)           | `01 00 04 00 00`           | **UNKNOWN** (device type?)   |
| `0x05` | `01 00`          | `01 00 01 00 01`           | **UNKNOWN** (feature flags?) |
| `0x07` | (none)           | `01`                       | **UNKNOWN** (SBX support?)   |
| `0x09` | `00`             | `00 03`                    | **UNKNOWN** (profile count?) |
| `0x0b` | `01 00 01 01 01` | `01 00 01 01 ff 00 00 ff`  | **UNKNOWN** (routing?)       |
| `0x0e` | `01`             | `01 01 00 00 00 00`        | **UNKNOWN** (EQ capability?) |
| `0x10` | (none)           | `00`                       | **UNKNOWN**                  |

#### RGB Lighting Control

```
OFF:        5a 3a 02 06 00
ON:         5a 3a 02 06 01
Mode:       5a 3a 06 04 00 03 01 00 01
Color:      5a 3a 09 0a 00 03 01 01 [R] [G] [B] [A]
```

RGB commands observed during late init. LED color data appears to be RGBA.
The on/off command (sub `0x06`) was mistakenly identified as "Capabilities Config" in earlier analysis.

---

### 0x3c — GainConfig

Dual-purpose command. Sub-commands control headphone amplifier gain and mic boost.

#### Read headphone gain (sub 0x02)

```
Send: 5a 3c 02 01 00
Recv: 5a 3c 04 01 00 [gain] 00
```

Observed: gain = `0x02`

**UNKNOWN:** Valid gain values, what they map to (impedance levels? gain stages?).

#### Read mic boost (sub 0x04)

```
Send: 5a 3c 04 01 00
Recv: 5a 3c [len] 01 00 [dB_u32_LE] ...
```

Same pattern as headphone gain read; value at bytes 5–8 is u32 LE (0, 10, 20, 30).

#### Write mic boost (sub 0x04)

```
Send: 5a 3c 04 00 00 02 [dB_u32_LE] 00 00 00 00 ...
```

| Value (u32 LE) | Mic boost |
|----------------|-----------|
| `0x00000000`   | 0 dB      |
| `0x0a000000`   | 10 dB     |
| `0x14000000`   | 20 dB     |
| `0x1e000000`   | 30 dB     |

---

### 0x6c — DacFilter

Controls CS43131 DAC digital reconstruction filter.
Uses the direction-flag pattern (byte 3): `0x00` = write, `0x01` = read response, `0x02` = enumerate.

#### Read current filter

```
Send: 5a 6c 01 01
Recv: 5a 6c 03 01 [filter] 00
```

#### Enumerate available filters

```
Send: 5a 6c 01 02
Recv: 5a 6c 0e 02 [count] [flags] 00 [id 00]×N
```

Flags byte `0x85` likely encodes: `0x80` = has enum, low bits = current filter.

#### Write filter selection

```
Send: 5a 6c 03 00 [filter] 00
```

**Filter values:**

| Value  | Filter                           |
|--------|----------------------------------|
| `0x01` | Fast Roll-off, Minimum Phase     |
| `0x02` | Slow Roll-off, Minimum Phase     |
| `0x03` | NOS (Non-Oversampling)           |
| `0x04` | Fast Roll-off, Linear Phase      |
| `0x05` | Slow Roll-off, Linear Phase      |

---

### 0x6e — Notification

Device-initiated hardware state change notification.

```
5a 6e 02 01 00
```

Observed once during output switching, and once during late init where it was sent host→device:
```
Send: 5a 6e 01 01
Recv: 5a 6e 02 01 00
```

**UNKNOWN:** Trigger conditions and full semantics.

---

## Family 0x96 — DSP Effects

Features are addressed by `(family, feature_id)` and read/written via commands `0x11` and `0x12`.
All values are IEEE 754 f32 little-endian.

### SBX Sub-Features

| ID     | Name             | Type     | Observed | Range / Values                      |
|--------|------------------|----------|----------|-------------------------------------|
| `0x00` | Surround Toggle  | Bool     | 0.0      | 0.0 / 1.0                          |
| `0x01` | Surround Level   | Float    | 0.12     | 0.0–1.0                            |
| `0x02` | Dialog+ Toggle   | Bool     | 0.0      | 0.0 / 1.0                          |
| `0x03` | Dialog+ Level    | Float    | 0.5      | 0.0–1.0                            |
| `0x04` | SmartVol Toggle  | Bool     | 1.0      | 0.0 / 1.0                          |
| `0x05` | SmartVol Level   | Float    | 0.5      | 0.0–1.0                            |
| `0x06` | SmartVol Mode    | Preset   | 0.0      | 0.0=Normal, 1.0=Loud, 2.0=Night    |
| `0x07` | Crystalizer Toggle| Bool    | 0.0      | 0.0 / 1.0                          |
| `0x08` | Crystalizer Level | Float   | 0.5      | 0.0–1.0                            |
| `0x17` | Surround Distance | Float   | 80.0     | 10.0–300.0, step 0.5               |
| `0x18` | Bass Toggle      | Bool     | 0.0      | 0.0 / 1.0                          |
| `0x19` | Bass Level       | Float    | 0.5      | 0.0–1.0                            |

### Equalizer

| ID     | Name      | Frequency | Range             | Step |
|--------|-----------|-----------|-------------------|------|
| `0x09` | EqToggle  | —         | 0.0 / 1.0 (bool) | —    |
| `0x0a` | EqPreAmp  | Pre-amp   | ±6.0 dB           | 0.5  |
| `0x0b` | Eq31Hz    | 31 Hz     | ±12.0 dB          | 0.5  |
| `0x0c` | Eq62Hz    | 62 Hz     | ±12.0 dB          | 0.5  |
| `0x0d` | Eq125Hz   | 125 Hz    | ±12.0 dB          | 0.5  |
| `0x0e` | Eq250Hz   | 250 Hz    | ±12.0 dB          | 0.5  |
| `0x0f` | Eq500Hz   | 500 Hz    | ±12.0 dB          | 0.5  |
| `0x10` | Eq1kHz    | 1 kHz     | ±12.0 dB          | 0.5  |
| `0x11` | Eq2kHz    | 2 kHz     | ±12.0 dB          | 0.5  |
| `0x12` | Eq4kHz    | 4 kHz     | ±12.0 dB          | 0.5  |
| `0x13` | Eq8kHz    | 8 kHz     | ±12.0 dB          | 0.5  |
| `0x14` | Eq16kHz   | 16 kHz    | ±12.0 dB          | 0.5  |

EQ dB ranges confirmed via BulkRangeDump (`0x15`).

### Unknown Region (0x96)

| ID     | Name          | Observed | Notes                        |
|--------|---------------|----------|------------------------------|
| `0x15` | **UNKNOWN**   | —        | Gap — never observed         |
| `0x16` | **UNKNOWN**   | —        | Gap — never observed         |
| `0x1a` | **UNKNOWN**   | 0.0      | Queried at startup, never written. Possibly mic-related. |
| `0x1b` | **UNKNOWN**   | 0.0      | Queried at startup, never written |
| `0x1c` | **UNKNOWN**   | 0.0      | Queried at startup, never written |
| `0x1d` | **UNKNOWN**   | 0.0      | Queried at startup, never written |
| `0x70` | **UNKNOWN**   | 0.0      | Seen in profile bulk transfers only |
| `0x71` | **UNKNOWN**   | 0.0      | Seen in profile bulk transfers only |
| `0x72` | **UNKNOWN**   | 0.0      | Seen in profile bulk transfers only |

---

## Family 0x95 — Recording Pane

Features for the recording pane (mic input): Voice Clarity, Acoustic Echo Cancellation, Mic EQ.
Read/written via standard `0x11`/`0x12` commands.

### Voice Clarity and Related

| ID     | Name                                | Type     | Values                                        |
|--------|-------------------------------------|----------|-----------------------------------------------|
| `0x00` | Acoustic Echo Cancellation          | Bool     | 0.0 / 1.0                                     |
| `0x04` | Voice Clarity / Noise Reduction     | Bool     | 0.0 / 1.0                                     |
| `0x05` | Noise Reduction Level               | Float    | continuous 0.0–0.5 (device scale)             |
| `0x13` | Mic EQ Enable                       | Bool     | 0.0 / 1.0                                     |
| `0x14`–`0x1b` | Mic EQ Band 1–8 (suspected)    | dB (f32) | Per-band gain, ±12 dB typical. **Not confirmed.** |
| `0x2c` | Recording Smart Volume              | Bool     | 0.0 / 1.0                                     |

**`0x04` — Voice Clarity / Noise Reduction toggle:** The Windows SBX software labels this feature as both "Voice Clarity" (the section toggle) and "Noise Reduction" (the on/off control within that section). Both UI actions write to the same register: `0.0` = OFF, `1.0` = ON. Writing to this feature triggers an unsolicited `GlobalProfileResponse` (0x26) from the device reflecting the current global state.

**`0x05` — Noise Reduction Level:** A continuous float on the device's `0.0–0.5` scale. The app converts to/from a `0.0–1.0` UI scale by halving on write and doubling on read. The device accepts any value in range — it is not limited to preset steps. Confirmed observed values (device scale):

| Device f32 | UI % (approx) | Hex (LE)     |
|------------|---------------|--------------|
| 0.0        | 0%            | `00 00 00 00` |
| 0.05       | 10%           | `cd cc 4c 3d` |
| 0.11       | 22%           | `ae 47 e1 3d` |
| 0.19       | 38%           | `5c 8f 42 3e` |
| 0.2        | 40%           | `cd cc 4c 3e` |
| 0.3        | 60%           | `9a 99 99 3e` |
| 0.37       | 74%           | `a4 70 bd 3e` |
| 0.4        | 80%           | `cd cc cc 3e` |
| 0.5        | 100%          | `00 00 00 3f` |

**Note:** The earlier observation that "20% maps to 0.05" is a label inconsistency in the g6-cli capture metadata, not a protocol quirk. The device stores the raw float and the UI scale is `device_f32 × 2.0`.

**Evidence for 0x14–0x1b as Mic EQ bands:** Packet captures show that when selecting a mic EQ preset, the software sends `5a 12 07 01 95 [14-1b] [f32_LE × 4]` with f32 dB values (e.g. `00 00 a0 c0` = -5.0 dB, `00 00 80 c0` = -4.0 dB, `00 00 40 40` ≈ +3.0 dB). These are suspected to be EQ band writes. The EQ sliders (bands 0x14–0x1b as individual controls) are **suspected, not confirmed**.

### Mic EQ Presets

*Not implemented yet — direct EQ sliders are planned first.*

Preset data from captures (8 f32 values for bands 0x14–0x1b, hex LE):

| Preset | 0x14 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19 | 0x1a | 0x1b |
|--------|------|------|------|------|------|------|------|------|
| 1 | 0000 40c0 | 0000 80c0 | 0000 0000 | 0000 4000 | 0000 4040 | 0000 40c0 | 0000 8040 | 0000 a040 |
| 2 | 0000 40c0 | 0000 80c0 | 0000 0000 | 0000 4000 | 0000 8040 | 0000 00c0 | 0000 0040 | 0000 8040 |
| 3 | 0000 00c0 | 0000 40c0 | 0000 4040 | 0000 8040 | 0000 8040 | 0000 80c0 | 0000 4040 | 0000 0040 |
| 4 | 0000 40c0 | 0000 a0c0 | 0000 0000 | 0000 8040 | 0000 0000 | 0000 40c0 | 0000 0000 | 0000 0000 |
| 5 | 0000 00c0 | 0000 40c0 | 0000 0040 | 0000 8040 | 0000 8040 | 0000 0000 | 0000 40c0 | 0000 0040 |
| 6 | 0000 a0c0 | 0000 80c0 | 0000 00c0 | 0000 0000 | 0000 4040 | 0000 8040 | 0000 c040 | 0000 e040 |
| 7 | 0000 0000 | 0000 4040 | 0000 00c0 | 0000 80c0 | 0000 80c0 | 0000 00c0 | 0000 a040 | 0000 e040 |
| 8 | 0000 0000 | 0000 0000 | 0000 0040 | 0000 0040 | 0000 4040 | 0000 80c0 | 0000 0040 | 0000 8040 |
| 9 | 0000 0000 | 0000 0000 | 0000 0040 | 0000 0040 | 0000 00c0 | 0000 0000 | 0000 80c0 | 0000 8040 |
| 10 | 0000 0000 | 0000 0040 | 0000 00c0 | 0000 0000 | 0000 4040 | 0000 a040 | 0000 c040 | 0000 a040 |
| DM_1 | 0000 0000 | 0000 0041 | 0000 0000 | 0000 4041 | 0000 4041 | 0000 8040 | 0000 0041 | 0000 2041 |

---

## How To: Common Operations

### Read any DSP feature value

```
Send: 5a 11 03 01 [family] [feature_id]
Recv: 5a 11 08 01 00 [family] [feature_id] [f32_LE × 4]
```

Parse bytes 7–10 as IEEE 754 f32 little-endian.

### Write any DSP feature value

```
Send: 5a 12 07 01 [family] [feature_id] [f32_LE × 4]
Recv: 5a 02 0a 12 00 ...           ← ACK
Recv: 5a 11 08 01 00 ...           ← unsolicited status push (optional to consume)
```

### Toggle an SBX effect (e.g. Crystalizer)

```
ON:  5a 12 07 01 96 07 00 00 80 3f    (1.0f)
OFF: 5a 12 07 01 96 07 00 00 00 00    (0.0f)
```

Same pattern for all toggles — just change the feature ID:
`0x00`=Surround, `0x02`=Dialog+, `0x04`=SmartVol, `0x07`=Crystalizer, `0x09`=EQ, `0x18`=Bass

### Set an SBX effect level (e.g. Crystalizer Level)

```
5a 12 07 01 96 08 [f32_LE]
```

Value is 0.0–1.0 (percentage). Same for `0x01`=Surround, `0x03`=Dialog+, `0x05`=SmartVol, `0x19`=Bass.

### Set SmartVolume Mode

```
5a 12 07 01 96 06 [f32_LE]
```

Values: `00 00 00 00` = Normal (0.0), `00 00 80 3f` = Loud (1.0), `00 00 00 40` = Night (2.0)

### Set an EQ band (e.g. 1kHz to +2.5 dB)

```
5a 12 07 01 96 10 00 00 20 40     (2.5f LE = 0x40200000)
```

### Query global state (SBX / Scout / EQ)

```
Send: 5a 26 03 08 ff ff
Recv: 5a 26 0b 08 ff ff [bitmask] 00 ...

bitmask & 0x01 → SBX Master ON
bitmask & 0x02 → Scout Mode ON    (mutually exclusive with SBX)
bitmask & 0x04 → EQ Enable ON     (independent)
```

### Toggle SBX Master

```
ON:  5a 26 05 07 01 00 01 00
OFF: 5a 26 05 07 01 00 00 00
```

### Toggle Scout Mode

```
ON:  5a 26 05 07 02 00 01 00
OFF: 5a 26 05 07 02 00 00 00
```

### Query current output mode

```
Send: 5a 2c 01 01
Recv: 5a 2c 05 01 [mode] 00 00 00

0x02 = Speakers
0x04 = Headphones
```

### Set output mode

```
5a 2c 05 00 [mode] 00 00 00
```

### Query current DAC filter

```
Send: 5a 6c 01 01
Recv: 5a 6c 03 01 [filter] 00

0x01 = Fast Roll-off, Minimum Phase
0x02 = Slow Roll-off, Minimum Phase
0x03 = NOS (Non-Oversampling)
0x04 = Fast Roll-off, Linear Phase
0x05 = Slow Roll-off, Linear Phase
```

### Set DAC filter

```
5a 6c 03 00 [filter] 00
```

### Query gain setting

```
Send: 5a 3c 02 01 00
Recv: 5a 3c 04 01 00 [gain] 00
```

---

## Open Questions

### Unidentified features
- **Family 0x96:** IDs 0x15, 0x16, 0x1a–0x1d, 0x70–0x72 — purpose unknown

### Commands not fully decoded
- **0x3a Capabilities:** Read-mode sub-commands not fully understood
- **0x3c GainConfig:** Headphone gain values and hardware mapping unknown
- **0x6e Notification:** Trigger conditions and full protocol unknown

### Protocol questions
- **Init repetition:** Why does SBX software run the full identification handshake 3–4 times?
- **0x26 bitmask bits 3–7:** Never observed set. Are there more global features on other Creative devices?
- **USB reset requirement:** Is there a gentler way to activate the interrupt endpoint on Linux without resetting the whole device?

---

## Appendix A: Known But Unused

Information decoded from captures but not currently implemented.

### Multi-Feature StatusResponse

The device can push multiple feature values in a single `0x11` packet. This happens during output switching
when the device dumps the entire profile for the new output.

**Length formula:** `byte[2] = 0x02 + (feature_count × 6)`

| Features | byte[2] |
|----------|---------|
| 1        | 0x08    |
| 2        | 0x0e    |
| 3        | 0x14    |
| 4        | 0x1a    |
| 7        | 0x2c    |
| 8        | 0x32    |

**Multi-feature packet structure:**
```
5a 11 [len] [count] 00 [family] [id] [f32_LE] [family] [id] [f32_LE] ...
```

**Observed profile dump during output switch (8 blocks):**
```
Block 1: 96:01 96:03 96:05 96:06 96:07 96:08 96:09 96:0a  (8 features, 0x32)
Block 2: 96:0b 96:0c 96:0d 96:0e 96:0f 96:10 96:11 96:12  (8 features, 0x32)
Block 3: 96:13 96:14 96:17 96:18 96:19 96:70 96:71 96:72  (8 features, 0x32)
Block 4: 95:00 95:04 95:05 95:0a 95:0b 95:0c 95:0d 95:0e  (8 features, 0x32)
Block 5: 95:0f 95:10 95:11 95:12 95:13 95:14 95:15 95:16  (8 features, 0x32)
Block 6: 95:17 95:18 95:19 95:1a 95:1b 95:1c 95:1d 95:1e  (8 features, 0x32)
Block 7: 95:1f 95:20 95:21 95:22 95:23 95:24 95:25 95:26  (8 features, 0x32)
Block 8: 95:27 95:28 95:29 95:2a 95:2b 95:2c 95:2d         (7 features, 0x2c)
```

---

### 0x39 — Direct Mode

Direct Mode bypasses SBX audio processing for bit-perfect audio output.

#### Enable

```
5a 39 03 00 05 01       → Direct Mode ON
5a 39 01 01             → Commit
```

#### Disable

```
5a 39 03 00 05 00       → Direct Mode OFF
5a 39 01 01             → Commit
```

Enabling Direct Mode also triggers `0x3c` (GainConfig) commands.
Disabling restores SBX state via explicit `0x12` writes (e.g., EQ toggle).

#### Read (sub 0x02, observed during init)

```
Send: 5a 39 01 02
Recv: 5a 39 07 02 20 64 02 04 00 00
```

Response data `20 64 02 04` not fully decoded. Possibly sample rate / format info.

Sub-commands `0x04` and `0x05` return ACK with `0x81` error (not supported on G6).

---

### Family 0x97 — Hardware / Decoder

| ID     | Name          | Type   | Values                    | Notes                    |
|--------|---------------|--------|---------------------------|--------------------------|
| `0x02` | Decoder Mode  | Preset | See below                 | Normal / Full / Night    |

**Decoder mode values (f32 LE):**

| Value | Hex (LE)     | Mode   |
|-------|---------------|--------|
| 2.0   | `00 00 00 40` | Normal |
| 1.0   | `00 00 80 3f` | Full   |
| 3.0   | `00 00 40 40` | Night  |
