//! Access to the G6's USB Audio Class mixer through ALSA.
//!
//! The G6 exposes its Mixer (volumes, mutes, monitoring, recording source)
//! as standard UAC Feature/Selector Units. The `snd-usb-audio` kernel driver
//! maps each of them to an ALSA control; reading or writing one sends a
//! standard USB control request (GET_CUR/SET_CUR) to the device.
//! Nothing here processes audio in software.

use alsa::mixer::{Mixer, Selem, SelemChannelId, SelemId};

use crate::features::AlsaDirection;

/// The kernel names the card after the device, independent of its index.
const CARD: &str = "hw:G6";

type Result<T> = std::result::Result<T, String>;

fn with_selem<T>(
    element: &str,
    action: impl FnOnce(&Selem) -> alsa::Result<T>,
) -> Result<T> {
    let mixer = Mixer::new(CARD, false)
        .map_err(|error| format!("cannot open mixer {}: {}", CARD, error))?;
    let selem = mixer
        .find_selem(&SelemId::new(element, 0))
        .ok_or_else(|| format!("mixer element '{}' not found", element))?;
    action(&selem).map_err(|error| format!("'{}': {}", element, error))
}

/// Whether the G6's ALSA card is present at all.
pub fn is_available() -> bool {
    Mixer::new(CARD, false).is_ok()
}

fn channel() -> SelemChannelId {
    SelemChannelId::FrontLeft
}

/// Volume as a fraction (0.0–1.0) of the control's raw range.
pub fn get_volume(element: &str, direction: AlsaDirection) -> Result<f32> {
    with_selem(element, |selem| {
        let ((min, max), volume) = match direction {
            AlsaDirection::Capture => (
                selem.get_capture_volume_range(),
                selem.get_capture_volume(channel())?,
            ),
            _ => (
                selem.get_playback_volume_range(),
                selem.get_playback_volume(channel())?,
            ),
        };
        Ok(fraction(volume, min, max))
    })
}

/// Sets all channels to `value` (0.0–1.0 of the raw range).
pub fn set_volume(
    element: &str,
    direction: AlsaDirection,
    value: f32,
) -> Result<()> {
    with_selem(element, |selem| match direction {
        AlsaDirection::Capture => {
            let (min, max) = selem.get_capture_volume_range();
            selem.set_capture_volume_all(raw(value, min, max))
        }
        _ => {
            let (min, max) = selem.get_playback_volume_range();
            selem.set_playback_volume_all(raw(value, min, max))
        }
    })
}

/// The gain in dB that a volume fraction corresponds to, for display.
pub fn volume_db(
    element: &str,
    direction: AlsaDirection,
    value: f32,
) -> Result<f32> {
    with_selem(element, |selem| {
        let millibel = match direction {
            AlsaDirection::Capture => {
                let (min, max) = selem.get_capture_volume_range();
                selem.ask_capture_vol_db(raw(value, min, max))?
            }
            _ => {
                let (min, max) = selem.get_playback_volume_range();
                selem.ask_playback_vol_db(raw(value, min, max))?
            }
        };
        Ok(millibel.to_db())
    })
}

/// 1.0 = on (unmuted), 0.0 = off (muted)
pub fn get_switch(element: &str, direction: AlsaDirection) -> Result<f32> {
    with_selem(element, |selem| {
        let state = match direction {
            AlsaDirection::Capture => selem.get_capture_switch(channel())?,
            _ => selem.get_playback_switch(channel())?,
        };
        Ok(if state != 0 { 1.0 } else { 0.0 })
    })
}

pub fn set_switch(
    element: &str,
    direction: AlsaDirection,
    value: f32,
) -> Result<()> {
    let state = if value > 0.0 { 1 } else { 0 };
    with_selem(element, |selem| match direction {
        AlsaDirection::Capture => selem.set_capture_switch_all(state),
        _ => selem.set_playback_switch_all(state),
    })
}

/// Index of the selected item of an enumerated element.
pub fn get_enum(element: &str) -> Result<f32> {
    with_selem(element, |selem| {
        Ok(selem.get_enum_item(channel())? as f32)
    })
}

pub fn set_enum(element: &str, value: f32) -> Result<()> {
    with_selem(element, |selem| {
        selem.set_enum_item(channel(), value as u32)
    })
}

fn fraction(volume: i64, min: i64, max: i64) -> f32 {
    if max <= min {
        return 0.0;
    }
    (volume - min) as f32 / (max - min) as f32
}

fn raw(value: f32, min: i64, max: i64) -> i64 {
    min + (value.clamp(0.0, 1.0) * (max - min) as f32).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_and_raw_round_trip() {
        for volume in 0..=146 {
            assert_eq!(raw(fraction(volume, 0, 146), 0, 146), volume);
        }
    }

    #[test]
    fn raw_clamps_out_of_range_values() {
        assert_eq!(raw(-0.5, 0, 57), 0);
        assert_eq!(raw(1.5, 0, 57), 57);
    }
}
