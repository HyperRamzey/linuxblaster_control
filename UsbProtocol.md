# SoundBlasterX G6 — USB HID Protocol Reference v5

*Reverse-engineered from USB captures, February 2026.*  
*v5 (October 2026): verified against the hardware with read queries and reversible write tests; see the notes marked **(verified)**.*

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
   - [0x26 — GlobalProfile (SBX/Scout/Voice Clarity)](#0x26--globalprofile)
   - [0x2c — OutputSelect](#0x2c--outputselect)
   - [0x30 — GetDspVersion](#0x30--getdspversion)
   - [0x3a — Capabilities / RGB Lighting](#0x3a--capabilities--rgb-lighting)
   - [0x3c — GainConfig (Mic Boost)](#0x3c--gainconfig)
   - [0x6c — DacFilter](#0x6c--dacfilter)
   - [0x6e — Notification](#0x6e--notification)
5. [Feature Family 0x96 — DSP Effects](#family-0x96--dsp-effects)
6. [Feature Family 0x95 — Recording Pane](#family-0x95--recording-pane)
7. [Feature Family 0x97 — Decoder](#family-0x97--decoder)
8. [Not HID: Mixer and Audio Quality](#not-hid-mixer-and-audio-quality)
9. [How To: Common Operations](#how-to-common-operations)
10. [Open Questions](#open-questions)
11. [Appendix A: Known But Unused](#appendix-a-known-but-unused)
    - [Multi-Feature StatusResponse](#multi-feature-statusresponse)
    - [0x39 — Direct Mode](#0x39--direct-mode)
12. [Appendix B: Unidentified Register Snapshot](#appendix-b-unidentified-register-snapshot)

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

### Linux: hidraw vs. libusb

**(verified)** Through the kernel's `hidraw` device (`/dev/hidrawN` for interface 4), the device answers
immediately after boot/plug-in: the `usbhid` driver keeps the interrupt IN endpoint polled.
No reset is needed. `hidapi` with its hidraw backend (`linux-static-hidraw`) uses this path.

With `hidapi`'s **libusb** backend (or pyusb/rusb), the kernel driver gets detached and the
interrupt pipe stays silent until the device receives a **USB reset** (`USBDEVFS_RESET`).
The reset affects the entire device (briefly interrupts audio), so the hidraw path is preferable.
Linuxblaster Control only falls back to the reset if the device doesn't answer a Ping (`0x06`).

To find the right hidraw node: it's the one whose parent USB interface ends in `:1.4`
(e.g. `readlink -f /sys/class/hidraw/hidraw2/device` → `...:1.4/...`).

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
  0x3c  Mic boost query
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
5a 02 0a [echoed_cmd] [status] [payload_echo or zeros]
```

Total payload length is always `0x0a` (10 bytes).

**Status byte** **(verified)**:

| Status | Meaning                                                                 |
|--------|-------------------------------------------------------------------------|
| `0x00` | OK                                                                      |
| `0x80` | Rejected, e.g. `5a 3c 02 01 01` (no gain control 0x01), `5a 3a 01 0a`   |
| `0x81` | Rejected, e.g. `5a 39 01 04` / `5a 39 01 05` (Direct Mode subs)         |
| `0x83` | Rejected, e.g. `5a 3a 01 04` (RGB mode sub without data)                |

Requests that return data on success (e.g. a `0x3c` read) answer with an ACK carrying an error status instead when they fail.

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

Controls and queries the global processing state: SBX Master and Scout Mode, and reports whether recording processing (Voice Clarity) is active.

#### Read bitmask

```
Send: 5a 26 03 08 ff ff
Recv: 5a 26 0b 08 ff ff [bitmask] 00 00 00 00 00 00 00
```

**Bitmask (verified):**

| Bit | Mask   | Meaning                              | Notes                                                    |
|-----|--------|--------------------------------------|----------------------------------------------------------|
| 0   | `0x01` | SBX / playback DSP active            | Set while **any** SBX effect or the EQ (`96:09`) is on   |
| 1   | `0x02` | Scout Mode                           | Mutually exclusive with SBX                              |
| 2   | `0x04` | Recording DSP active (Voice Clarity) | Follows Noise Reduction `95:04`; **not** EQ Enable       |

Earlier versions of this document labeled bit 2 "EQ Enable". Hardware tests show otherwise:
writing `96:09 = 0` (EQ off, all other effects off) changes `0x05 → 0x04` (bit 0 cleared),
and writing `95:04 = 0` (Noise Reduction off) changes `0x05 → 0x01` (bit 2 cleared).

**Observed values:**
- `0x05` = SBX on + Voice Clarity on
- `0x04` = SBX off + Voice Clarity on
- `0x06` = Scout on + Voice Clarity on (SBX auto-disabled)
- `0x01` = SBX on + Voice Clarity off

#### SBX Master semantics (verified)

- **SBX off** (`5a 26 05 07 01 00 00 00`): the device switches all playback effects off and reports their toggles as `0.0`.
  It remembers which effects were on.
- **SBX on** right after SBX off: the remembered effects come back.
- **SBX on** when no effect is remembered (e.g. the effects were switched off individually,
  or a single effect was written while SBX was off): the device enables **Surround** (`96:00`) by itself.
- **Writing an effect toggle** (`5a 12 ... 96 [toggle] 1.0`) while SBX is off turns on only that effect
  and sets bit 0. SBX doesn't need to be switched on first, and doing so would cause the Surround side effect above.
- Switching the last effect off clears bit 0, so SBX reads as off.
- Every change is followed by an unsolicited `0x26` response with the new bitmask, often before the ACK.

#### Write toggles

```
SBX Master:  5a 26 05 07 01 00 [00|01] 00
Scout Mode:  5a 26 05 07 02 00 [00|01] 00
```

EQ Enable is a regular feature write: `5a 12 07 01 96 09 [f32]`

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
| `0x07` | (none)           | `01`                       | Lighting enabled (see below) |
| `0x09` | `00`             | `00 03`                    | **UNKNOWN** (profile count?) |
| `0x0b` | `01 00 01 01 01` | `01 00 01 01 ff 00 00 ff`  | Lighting color `[A B G R]`   |
| `0x0e` | `01`             | `01 01 00 00 00 00`        | **UNKNOWN** (EQ capability?) |
| `0x10` | (none)           | `00`                       | **UNKNOWN**                  |

#### RGB Lighting Control (verified)

```
OFF:        5a 3a 02 06 00
ON:         5a 3a 02 06 01
Mode:       5a 3a 06 04 00 03 01 00 01                 (static color)
Color:      5a 3a 09 0a 00 03 01 01 [A] [B] [G] [R]
```

The color is **`[A][B][G][R]`**, not RGBA. The Windows software always sends `A = 0xff`.
Examples: red = `ff 00 00 ff`, green = `ff 00 ff 00`, yellow = `ff 00 ff ff`, white = `ff ff ff ff`.
The Windows software sends each packet three times; once is enough.

Note: `5a 3a 01 06` (sub `0x06` with no data byte) is accepted and switches the LED **off**.

#### Reading the lighting state (verified)

```
Enabled:    Send 5a 3a 01 07                  → Recv 5a 3a 02 07 [0|1]
Color:      Send 5a 3a 06 0b 01 00 01 01 01   → Recv 5a 3a 09 0b 01 00 01 01 [A] [B] [G] [R]
```

Sub `0x07` and sub `0x0b` from the capability table below are the lighting state reads.

The on/off command (sub `0x06`) was mistakenly identified as "Capabilities Config" in earlier analysis.

---

### 0x3c — GainConfig

Gain controls. The enumerate request lists exactly **one** control, id `0x02`, which is the **mic boost** **(verified)**.
Earlier versions read `5a 3c 04 01 00 02 00` as "headphone gain = 0x02". The `0x02` is the control id, and the trailing `00` is the boost (0 dB).

Same direction-flag pattern as `0x2c`/`0x6c` (byte 3): `0x00` = write, `0x01` = read, `0x02` = enumerate.

#### Read mic boost

```
Send: 5a 3c 02 01 00
Recv: 5a 3c 04 01 00 02 [dB]
```

The byte after `00` selects the control. Any other value (`01`, `02`, ...) is rejected with ACK status `0x80`.

#### Write mic boost

```
Send: 5a 3c 04 00 00 02 [dB]
Recv: 5a 02 0a 3c 00 00 00 02 [dB] ...    ← ACK echoes the payload
```

| `[dB]` | Mic boost |
|--------|-----------|
| `0x00` | 0 dB      |
| `0x0a` | 10 dB     |
| `0x14` | 20 dB     |
| `0x1e` | 30 dB     |

The Windows software sends the write followed by four reads.

#### Enumerate

```
Send: 5a 3c 01 02
Recv: 5a 3c 09 02 01 01 00 00 02 00 00 00
```

`01` = one control; the entry contains id `02`. The remaining bytes are not decoded.

The ALSA control `Input Gain Control` (range 0–3) does **not** follow the mic boost.

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
| `0x14`–`0x1b` | Mic EQ Band 1–8                | dB (f32) | Per-band gain, ±12 dB. **(verified)**         |
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

**0x14–0x1b are the Mic EQ bands (verified):** selecting a mic EQ preset makes the software write `5a 12 07 01 95 [14-1b] [f32_LE × 4]` with dB values, and the device reads back exactly the values of the last selected preset (Preset 1 on the test device). The band center frequencies are unknown. The Windows UI only offers presets.

### Mic EQ Presets

Preset data from captures (8 f32 values for bands 0x14–0x1b, hex LE).
Decoded dB values are in `MIC_EQ_PRESETS` in `src/features.rs`.

| Preset | 0x14 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19 | 0x1a | 0x1b |
|--------|------|------|------|------|------|------|------|------|
| 1 | 0000 40c0 | 0000 80c0 | 0000 0000 | 0000 0040 | 0000 4040 | 0000 40c0 | 0000 8040 | 0000 a040 |
| 2 | 0000 40c0 | 0000 80c0 | 0000 0000 | 0000 0040 | 0000 8040 | 0000 00c0 | 0000 0040 | 0000 8040 |
| 3 | 0000 00c0 | 0000 40c0 | 0000 4040 | 0000 8040 | 0000 8040 | 0000 80c0 | 0000 4040 | 0000 0040 |
| 4 | 0000 40c0 | 0000 a0c0 | 0000 0000 | 0000 8040 | 0000 0000 | 0000 40c0 | 0000 0000 | 0000 0000 |
| 5 | 0000 00c0 | 0000 40c0 | 0000 0040 | 0000 8040 | 0000 8040 | 0000 0000 | 0000 40c0 | 0000 0040 |
| 6 | 0000 a0c0 | 0000 80c0 | 0000 00c0 | 0000 0000 | 0000 4040 | 0000 8040 | 0000 c040 | 0000 e040 |
| 7 | 0000 0000 | 0000 4040 | 0000 00c0 | 0000 80c0 | 0000 80c0 | 0000 00c0 | 0000 a040 | 0000 e040 |
| 8 | 0000 0000 | 0000 0000 | 0000 0040 | 0000 0040 | 0000 4040 | 0000 80c0 | 0000 0040 | 0000 8040 |
| 9 | 0000 0000 | 0000 0000 | 0000 0040 | 0000 0040 | 0000 00c0 | 0000 0000 | 0000 80c0 | 0000 8040 |
| 10 | 0000 0000 | 0000 0040 | 0000 00c0 | 0000 0000 | 0000 4040 | 0000 a040 | 0000 c040 | 0000 a040 |
| DM_1 | 0000 0000 | 0000 0041 | 0000 0000 | 0000 4041 | 0000 4041 | 0000 8040 | 0000 0041 | 0000 2041 |

The band 4 entries of presets 1 and 2 were listed as `0000 4000` in earlier versions (inherited from the g6-cli tables).
That would be a denormal ≈ 0; the device stores `0000 0040` = +2.0 dB.

---

## Family 0x97 — Decoder

| ID     | Name          | Type   | Values                    |
|--------|---------------|--------|---------------------------|
| `0x02` | Decoder Mode  | Preset | See below **(verified)**  |

**Decoder mode values (f32 LE):**

| Value | Hex (LE)      | Mode   |
|-------|---------------|--------|
| 2.0   | `00 00 00 40` | Normal |
| 1.0   | `00 00 80 3f` | Full   |
| 3.0   | `00 00 40 40` | Night  |

Read/written with the standard `0x11`/`0x12` commands (`5a 11 03 01 97 02`, `5a 12 07 01 97 02 [f32]`).
A read sweep of family 0x97 (`0x00`–`0x7f`) only answers for `0x02`.

---

## Not HID: Mixer and Audio Quality

### Mixer (USB Audio Class)

The Windows "Mixer" tab, plus the recording/monitoring volume in the Recording tab, don't use the HID protocol at all.
They are standard **USB Audio Class** Feature Unit / Selector Unit requests on the AudioControl interface,
which is why captures of those settings contain no HID packets.

On Linux, `snd-usb-audio` exposes these hardware controls as ALSA mixer controls on the card with id `G6`.
Writing one sends the same `SET_CUR` request Windows sends. Nothing is processed in software.

| Windows UI                         | UAC unit (`wIndex` high byte) | ALSA simple control                       |
|------------------------------------|-------------------------------|-------------------------------------------|
| Playback: Speakers                 | `0x01`                        | `Speaker` (playback volume/switch)        |
| Monitoring: Line In                | `0x09`                        | `Line In` (playback)                      |
| Monitoring: External Mic           | `0x0a`                        | `External Mic` (playback)                 |
| Monitoring: S/PDIF In              | `0x0c`                        | `S/PDIF In` (playback)                    |
| Recording: Line In                 | `0x03`                        | `Line In` (capture)                       |
| Recording: External Mic            | `0x04`                        | `External Mic` (capture)                  |
| Recording: S/PDIF In               | `0x05`                        | `S/PDIF In` (capture)                     |
| Recording: What U Hear             | `0x06`                        | `What U Hear` (capture)                   |
| Recording source                   | Selector Unit                 | `PCM Capture Source` (enum)               |

Unit numbers are from the g6-cli captures (`bRequest 0x01 SET_CUR`, `wValue 0x0100` = mute, `0x0201/0x0202` = volume left/right).

### Audio Quality (bit depth / sample rate)

Not a device setting. The sample format is chosen per stream by whoever opens the PCM device
(on Linux: PipeWire/PulseAudio/JACK, or the application itself through ALSA).
Captures of changing it in Windows contain no HID traffic.

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

### Query global state (SBX / Scout / Voice Clarity)

```
Send: 5a 26 03 08 ff ff
Recv: 5a 26 0b 08 ff ff [bitmask] 00 ...

bitmask & 0x01 → SBX on (any playback effect / EQ active)
bitmask & 0x02 → Scout Mode ON    (mutually exclusive with SBX)
bitmask & 0x04 → Voice Clarity (Noise Reduction) ON
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

### Query / set mic boost

```
Send: 5a 3c 02 01 00
Recv: 5a 3c 04 01 00 02 [dB]       (0x00, 0x0a, 0x14, 0x1e)

Set:  5a 3c 04 00 00 02 [dB]
```

### Lighting

```
On/off:  5a 3a 02 06 [00|01]
Color:   5a 3a 06 04 00 03 01 00 01
         5a 3a 09 0a 00 03 01 01 ff [B] [G] [R]
```

### Decoder mode

```
5a 12 07 01 97 02 [f32_LE]     2.0 = Normal, 1.0 = Full, 3.0 = Night
```

---

## Open Questions

### Unidentified features
- **Family 0x96:** IDs 0x15, 0x16, 0x1a–0x1d, 0x3d, 0x3f, 0x70–0x72 — purpose unknown (see [Appendix B](#appendix-b-unidentified-register-snapshot))
- **Family 0x95:** IDs 0x01–0x03, 0x06–0x12 — purpose unknown (several look like Voice Clarity / AEC parameters)
- **Mic EQ band frequencies** (0x95:0x14–0x1b)
- **"Physical Speakers Configuration"** in the Windows Playback tab: g6-cli captured "Desktop Speakers" as `96:18 = 0.0` and `96:17 = 150.0`. These are the Bass toggle and Surround Distance addresses, so this preset seems to just set existing features.

### Commands not fully decoded
- **0x3a Capabilities:** sub-commands `0x00`, `0x05`, `0x09`, `0x0e`, `0x10` not understood; the lighting mode packet's parameters (`04 00 03 01 00 01`) are only known for static color
- **0x6e Notification:** Trigger conditions and full protocol unknown

### Protocol questions
- **Init repetition:** Why does SBX software run the full identification handshake 3–4 times?
- **0x26 bitmask bits 3–7:** Never observed set. Are there more global features on other Creative devices?
- ~~**USB reset requirement:** Is there a gentler way to activate the interrupt endpoint on Linux without resetting the whole device?~~ Yes: use hidraw instead of libusb (see [Transport Layer](#linux-hidraw-vs-libusb)).
- **Hardware buttons:** Which unsolicited packets do the SBX / Scout Mode buttons on the device produce? (Linuxblaster Control already applies pushed `0x11`, `0x26` and `0x2c` packets.)

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

---

## Appendix B: Unidentified Register Snapshot

Read-only sweep (`5a 11 03 01 [family] [id]` for ids `0x00`–`0x7f`) of a G6 with firmware `2.1.250903.1324`,
headphone output, SBX on (EQ only), Noise Reduction on. Only answers are listed; everything not listed reads `0.0`
(family 0x95 answers up to `0x38`, family 0x96 up to `0x41`, family 0x97 only `0x02`).
Known registers are omitted.

| Register | Value  | Notes                                                        |
|----------|--------|--------------------------------------------------------------|
| `95:01`  | 0.23   |                                                              |
| `95:07`  | 0.05   |                                                              |
| `95:08`  | 30.0   |                                                              |
| `95:09`  | 30.0   |                                                              |
| `95:0b`  | 400.0  | looks like a frequency (Hz)                                  |
| `95:0c`  | 1400.0 | looks like a frequency (Hz)                                  |
| `95:0d`  | 2000.0 | looks like a frequency (Hz)                                  |
| `95:0e`  | 1.0    |                                                              |
| `95:0f`  | 1.0    |                                                              |
| `95:10`  | 1.0    |                                                              |
| `96:16`  | 80.0   | same as Surround Distance `96:17`                            |
| `96:3d`  | 1.0    |                                                              |
| `96:3f`  | 2.0    |                                                              |
