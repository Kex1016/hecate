//! CRTC gamma for wlr-gamma-control on the tty backend.
//!
//! Adapted from niri (src/backend/tty.rs, `GammaProps`), GPL-3.0-or-later.

use std::iter::zip;
use std::num::NonZeroU64;
use std::os::fd::AsFd;

use anyhow::{Context, ensure};
use smithay::backend::drm::DrmDevice;
use smithay::reexports::drm::control::{Device as _, crtc, property};

pub struct GammaProps {
    crtc: crtc::Handle,
    mode: GammaMode,
}

enum GammaMode {
    /// GAMMA_LUT property.
    Lut {
        gamma_lut: property::Handle,
        gamma_lut_size: u32,
        previous_blob: Option<NonZeroU64>,
    },
    /// Legacy set_gamma() call.
    Legacy {
        gamma_size: u32,
        previous_ramp: Option<Vec<u16>>,
    },
    /// Approximation through the CTM property, the last resort (Asahi).
    Ctm {
        ctm: property::Handle,
        previous_blob: Option<NonZeroU64>,
    },
}

impl GammaProps {
    pub fn new(device: &DrmDevice, crtc: crtc::Handle) -> anyhow::Result<Self> {
        let mut gamma_lut = None;
        let mut gamma_lut_size = None;
        let mut ctm = None;

        let props = device.get_properties(crtc).context("error getting properties")?;
        for (prop, value) in props {
            let Ok(info) = device.get_property(prop) else {
                continue;
            };
            let Ok(name) = info.name().to_str() else {
                continue;
            };
            match name {
                "GAMMA_LUT" if matches!(info.value_type(), property::ValueType::Blob) => {
                    gamma_lut = Some(prop)
                }
                "GAMMA_LUT_SIZE" if matches!(info.value_type(), property::ValueType::UnsignedRange(_, _)) => {
                    gamma_lut_size = Some(value as u32)
                }
                "CTM" if matches!(info.value_type(), property::ValueType::Blob) => ctm = Some(prop),
                _ => (),
            }
        }

        let mode = if let (Some(gamma_lut), Some(gamma_lut_size)) = (gamma_lut, gamma_lut_size) {
            GammaMode::Lut {
                gamma_lut,
                gamma_lut_size,
                previous_blob: None,
            }
        } else {
            let info = device.get_crtc(crtc).context("error getting crtc info")?;
            let gamma_size = info.gamma_length();
            if gamma_size != 0 {
                GammaMode::Legacy {
                    gamma_size,
                    previous_ramp: None,
                }
            } else {
                let ctm = ctm.context("setting gamma is not supported")?;
                tracing::debug!("no GAMMA_LUT or legacy gamma; approximating gamma with CTM");
                GammaMode::Ctm {
                    ctm,
                    previous_blob: None,
                }
            }
        };

        Ok(Self { crtc, mode })
    }

    pub fn gamma_size(&self) -> u32 {
        match &self.mode {
            GammaMode::Lut { gamma_lut_size, .. } => *gamma_lut_size,
            GammaMode::Legacy { gamma_size, .. } => *gamma_size,
            GammaMode::Ctm { .. } => 256,
        }
    }

    pub fn set_gamma(&mut self, device: &DrmDevice, gamma: Option<Vec<u16>>) -> anyhow::Result<()> {
        let is_lut = matches!(self.mode, GammaMode::Lut { .. });
        let gamma_size = self.gamma_size() as usize;
        let (prop, prop_name) = match &mut self.mode {
            GammaMode::Lut { gamma_lut, .. } => (*gamma_lut, "GAMMA_LUT"),
            GammaMode::Ctm { ctm, .. } => (*ctm, "CTM"),
            GammaMode::Legacy {
                gamma_size,
                previous_ramp,
            } => {
                set_gamma_for_crtc(device, self.crtc, *gamma_size, gamma.as_deref())?;
                *previous_ramp = gamma;
                return Ok(());
            }
        };

        let blob = if let Some(gamma) = gamma {
            ensure!(gamma.len() == gamma_size * 3, "wrong gamma length");
            let (red, rest) = gamma.split_at(gamma_size);
            let (green, blue) = rest.split_at(gamma_size);

            let mut data: Vec<u8> = if is_lut {
                // struct drm_color_lut { u16 red, green, blue, reserved }
                zip(zip(red, green), blue)
                    .flat_map(|((&r, &g), &b)| [r, g, b, 0])
                    .flat_map(u16::to_ne_bytes)
                    .collect()
            } else {
                // struct drm_color_ctm { u64 matrix[9] } in S31.32, a linear approximation of the
                // ramp (see kwin f2417a85 and melissawen's steamdeck-colors post).
                fn s31_32(value: u16) -> u64 {
                    let normalized = value as f64 / u16::MAX as f64;
                    (normalized * (1u64 << 32) as f64) as u64
                }
                let last = gamma_size - 1;
                [
                    s31_32(red[last]),
                    0,
                    0,
                    0,
                    s31_32(green[last]),
                    0,
                    0,
                    0,
                    s31_32(blue[last]),
                ]
                .into_iter()
                .flat_map(u64::to_ne_bytes)
                .collect()
            };
            let blob = drm_ffi::mode::create_property_blob(device.as_fd(), &mut data)
                .context("error creating property blob")?;
            NonZeroU64::new(u64::from(blob.blob_id))
        } else {
            None
        };

        let raw = blob.map(NonZeroU64::get).unwrap_or(0);
        if let Err(err) = device.set_property(self.crtc, prop, property::Value::Blob(raw).into()) {
            if raw != 0
                && let Err(err) = device.destroy_property_blob(raw)
            {
                tracing::warn!("error destroying {prop_name} blob: {err:?}");
            }
            return Err(err).with_context(|| format!("error setting {prop_name}"));
        }

        if let GammaMode::Lut { previous_blob, .. } | GammaMode::Ctm { previous_blob, .. } = &mut self.mode
            && let Some(old) = std::mem::replace(previous_blob, blob)
            && let Err(err) = device.destroy_property_blob(old.get())
        {
            tracing::warn!("error destroying previous {prop_name} blob: {err:?}");
        }
        Ok(())
    }

    /// Re-apply the current gamma (after VT switch back).
    pub fn restore_gamma(&self, device: &DrmDevice) -> anyhow::Result<()> {
        match &self.mode {
            GammaMode::Lut {
                gamma_lut,
                previous_blob,
                ..
            } => {
                let blob = previous_blob.map(NonZeroU64::get).unwrap_or(0);
                device
                    .set_property(self.crtc, *gamma_lut, property::Value::Blob(blob).into())
                    .context("error setting GAMMA_LUT")?;
            }
            GammaMode::Ctm { ctm, previous_blob } => {
                let blob = previous_blob.map(NonZeroU64::get).unwrap_or(0);
                device
                    .set_property(self.crtc, *ctm, property::Value::Blob(blob).into())
                    .context("error setting CTM")?;
            }
            GammaMode::Legacy {
                gamma_size,
                previous_ramp,
            } => set_gamma_for_crtc(device, self.crtc, *gamma_size, previous_ramp.as_deref())?,
        }
        Ok(())
    }
}

/// Linear ramp of `len` entries per channel.
pub fn linear_ramp(len: usize) -> Vec<u16> {
    let denom = (len as u64).saturating_sub(1).max(1);
    let channel: Vec<u16> = (0..len).map(|i| (0xFFFF * i as u64 / denom) as u16).collect();
    channel.repeat(3)
}

fn set_gamma_for_crtc(
    device: &DrmDevice,
    crtc: crtc::Handle,
    gamma_length: u32,
    ramp: Option<&[u16]>,
) -> anyhow::Result<()> {
    let len = gamma_length as usize;
    // The legacy API can't reset gamma, so set a linear ramp.
    let linear;
    let ramp = match ramp {
        Some(r) => {
            ensure!(r.len() == len * 3, "wrong gamma length");
            r
        }
        None => {
            linear = linear_ramp(len);
            &linear
        }
    };
    let (red, rest) = ramp.split_at(len);
    let (green, blue) = rest.split_at(len);
    device
        .set_gamma(crtc, red, green, blue)
        .context("error setting legacy gamma")
}

#[cfg(test)]
mod tests {
    #[test]
    fn linear_ramp_spans_full_range() {
        let r = super::linear_ramp(256);
        assert_eq!(r.len(), 768);
        assert_eq!(r[0], 0);
        assert_eq!(r[255], 0xFFFF);
        assert_eq!(r[256], 0);
        assert_eq!(r[767], 0xFFFF);
    }
}
