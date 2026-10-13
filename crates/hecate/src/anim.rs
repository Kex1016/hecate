//! Animation timing: a clock, cubic-bézier easing and running animations.

use std::time::Duration;

use crate::config::{Anim, AnimStyle, Animations};

/// Time source for animations. Tests freeze it and step it by hand.
#[derive(Debug, Default)]
pub struct Clock {
    manual: Option<Duration>,
    /// Every animation finishes as soon as it starts (tests that don't care about animations).
    pub instant: bool,
}

impl Clock {
    pub fn now(&self) -> Duration {
        self.manual.unwrap_or_else(crate::utils::monotonic_time)
    }

    /// Freeze the clock at `t` (tests).
    pub fn set_manual(&mut self, t: Duration) {
        self.manual = Some(t);
        self.instant = false;
    }

    /// Step a frozen clock (tests).
    pub fn advance(&mut self, by: Duration) {
        let now = self.now();
        self.manual = Some(now + by);
    }
}

/// A cubic bézier from (0, 0) to (1, 1) with control points (x1, y1) and (x2, y2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve(pub [f64; 4]);

impl Curve {
    pub const LINEAR: Curve = Curve([0., 0., 1., 1.]);

    fn axis(t: f64, p1: f64, p2: f64) -> f64 {
        let u = 1. - t;
        3. * u * u * t * p1 + 3. * u * t * t * p2 + t * t * t
    }

    fn axis_deriv(t: f64, p1: f64, p2: f64) -> f64 {
        let u = 1. - t;
        3. * u * u * p1 + 6. * u * t * (p2 - p1) + 3. * t * t * (1. - p2)
    }

    /// Eased value for linear progress `x` in [0, 1] (may leave [0, 1] for overshooting curves).
    pub fn ease(&self, x: f64) -> f64 {
        let [x1, y1, x2, y2] = self.0;
        let x = x.clamp(0., 1.);
        if x == 0. || x == 1. {
            return x;
        }
        // Newton's method on x(t) = x, falling back to bisection (x(t) is monotonic for x1, x2
        // in [0, 1], which the config enforces by clamping).
        let mut t = x;
        for _ in 0..8 {
            let err = Self::axis(t, x1, x2) - x;
            if err.abs() < 1e-7 {
                return Self::axis(t, y1, y2);
            }
            let d = Self::axis_deriv(t, x1, x2);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0., 1.);
        t = x;
        for _ in 0..40 {
            let v = Self::axis(t, x1, x2);
            if (v - x).abs() < 1e-7 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.;
        }
        Self::axis(t, y1, y2)
    }
}

/// Animation state of a layer surface (in its user data).
#[derive(Debug, Default)]
pub struct LayerAnimState {
    /// Open animation, with the offset it slides in from.
    pub open: Option<(Animation, smithay::utils::Point<f64, smithay::utils::Logical>)>,
    /// Mapped with a buffer (an unmap and remap opens again).
    pub shown: bool,
}

/// One running animation: eased progress from 0 to 1 over a duration.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    start: Duration,
    duration: Duration,
    curve: Curve,
    pub style: AnimStyle,
    /// Starting scale for popin.
    pub from: f64,
    pub fade: bool,
}

impl Animation {
    /// Start an animation now, or None if animations are off or it has nothing to show.
    pub fn start(clock: &Clock, config: &Animations, anim: &Anim) -> Option<Animation> {
        if !config.enable || clock.instant || anim.style == AnimStyle::None {
            return None;
        }
        let ms = (anim.ms as f64 * config.slowdown.max(0.)).round();
        if ms <= 0. {
            return None;
        }
        let curve = config
            .curves
            .get(&anim.curve)
            .map(|&[x1, y1, x2, y2]| Curve([x1.clamp(0., 1.), y1, x2.clamp(0., 1.), y2]))
            .unwrap_or(Curve::LINEAR);
        Some(Animation {
            start: clock.now(),
            duration: Duration::from_millis(ms as u64),
            curve,
            style: anim.style,
            from: anim.from,
            fade: anim.fade,
        })
    }

    /// Linear progress in [0, 1].
    pub fn linear(&self, now: Duration) -> f64 {
        let t = now.saturating_sub(self.start).as_secs_f64();
        (t / self.duration.as_secs_f64()).clamp(0., 1.)
    }

    /// Eased progress.
    pub fn value(&self, now: Duration) -> f64 {
        self.curve.ease(self.linear(now))
    }

    pub fn is_done(&self, now: Duration) -> bool {
        now >= self.start + self.duration
    }

    /// Scale and opacity for an opening (`closing = false`) or closing element.
    pub fn scale_alpha(&self, now: Duration, closing: bool) -> (f64, f32) {
        let v = self.value(now);
        // Opening runs 0 → 1, closing 1 → 0.
        let p = if closing { 1. - v } else { v };
        let scale = match self.style {
            AnimStyle::Popin => self.from + (1. - self.from) * p,
            _ => 1.,
        };
        let fade = self.fade || self.style == AnimStyle::Fade;
        // Overshooting curves shouldn't push opacity past fully opaque.
        let alpha = if fade { p.clamp(0., 1.) as f32 } else { 1. };
        (scale, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_curve_is_identity() {
        for x in [0., 0.1, 0.25, 0.5, 0.9, 1.] {
            assert!((Curve::LINEAR.ease(x) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn ease_endpoints_and_monotonic_decel() {
        let c = Curve([0.05, 0.7, 0.1, 1.0]);
        assert_eq!(c.ease(0.), 0.);
        assert_eq!(c.ease(1.), 1.);
        // Decelerating: well ahead of linear early on, and never going backwards.
        assert!(c.ease(0.2) > 0.5);
        let mut last = 0.;
        for i in 0..=100 {
            let v = c.ease(i as f64 / 100.);
            assert!(v + 1e-9 >= last, "not monotonic at {i}");
            last = v;
        }
    }

    #[test]
    fn overshooting_curve_leaves_range() {
        let c = Curve([0.42, 1.67, 0.21, 0.90]);
        let max = (0..=100).map(|i| c.ease(i as f64 / 100.)).fold(0., f64::max);
        assert!(max > 1.0, "expressive spatial curve overshoots");
    }

    #[test]
    fn animation_progress_and_end() {
        let mut clock = Clock::default();
        clock.set_manual(Duration::from_secs(10));
        let cfg = Animations::default();
        let anim = Anim {
            style: AnimStyle::Popin,
            from: 0.5,
            ms: 100,
            curve: "linear".into(),
            fade: true,
        };
        let a = Animation::start(&clock, &cfg, &anim).unwrap();
        assert_eq!(a.scale_alpha(clock.now(), false), (0.5, 0.));
        clock.advance(Duration::from_millis(50));
        let (s, al) = a.scale_alpha(clock.now(), false);
        assert!((s - 0.75).abs() < 1e-6 && (al - 0.5).abs() < 1e-6);
        assert!(!a.is_done(clock.now()));
        clock.advance(Duration::from_millis(50));
        assert!(a.is_done(clock.now()));
        assert_eq!(a.scale_alpha(clock.now(), false), (1.0, 1.0));
        assert_eq!(a.scale_alpha(clock.now(), true), (0.5, 0.0));
    }

    #[test]
    fn disabled_or_instant_starts_nothing() {
        let mut clock = Clock::default();
        clock.set_manual(Duration::ZERO);
        let anim = Anim::default();
        let off = Animations {
            enable: false,
            ..Default::default()
        };
        assert!(Animation::start(&clock, &off, &anim).is_none());
        let none = Anim {
            style: AnimStyle::None,
            ..Default::default()
        };
        assert!(Animation::start(&clock, &Animations::default(), &none).is_none());
        clock.instant = true;
        assert!(Animation::start(&clock, &Animations::default(), &anim).is_none());
    }

    #[test]
    fn slowdown_scales_duration() {
        let mut clock = Clock::default();
        clock.set_manual(Duration::ZERO);
        let cfg = Animations {
            slowdown: 2.0,
            ..Default::default()
        };
        let anim = Anim {
            ms: 100,
            ..Default::default()
        };
        let a = Animation::start(&clock, &cfg, &anim).unwrap();
        clock.advance(Duration::from_millis(150));
        assert!(!a.is_done(clock.now()));
        clock.advance(Duration::from_millis(50));
        assert!(a.is_done(clock.now()));
    }
}
