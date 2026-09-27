//! Spring design parameters; frame scheduling and integration belong to the UI framework.

use std::{f32::consts::PI, time::Duration};

/// A finite, damped spring with unit mass.
///
/// Duration is a response parameter, not a deadline for reaching rest. These presets
/// follow the public duration/bounce convention, not measured glass-control motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    stiffness: f32,
    damping: f32,
}

/// An invalid duration/bounce spring description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SpringError {
    /// A zero duration cannot produce finite physical parameters.
    #[error("spring duration must be positive")]
    ZeroDuration,
    /// Bounce must produce finite, positive damping.
    #[error("spring bounce must be finite and strictly between -1 and 1")]
    InvalidBounce,
}

impl Spring {
    /// A critically damped spring with a response duration of 500 ms.
    pub const SMOOTH: Self = Self {
        stiffness: 16.0 * PI * PI,
        damping: 8.0 * PI,
    };

    /// A spring with a response duration of 500 ms and bounce of 0.15.
    pub const SNAPPY: Self = Self {
        stiffness: Self::SMOOTH.stiffness,
        damping: 8.0 * PI * 0.85,
    };

    /// A spring with a response duration of 500 ms and bounce of 0.30.
    pub const BOUNCY: Self = Self {
        stiffness: Self::SMOOTH.stiffness,
        damping: 8.0 * PI * 0.70,
    };

    /// Converts response duration and bounce into finite physical parameters.
    ///
    /// Negative bounce gives overdamping, zero gives critical damping, and positive
    /// bounce gives underdamping. Zero duration and bounce outside `(-1, 1)` are errors;
    /// request an immediate update through the UI framework instead of a zero-duration spring.
    pub fn new(duration: Duration, bounce: f32) -> Result<Self, SpringError> {
        if duration.is_zero() {
            return Err(SpringError::ZeroDuration);
        }
        if !bounce.is_finite() || bounce <= -1.0 || bounce >= 1.0 {
            return Err(SpringError::InvalidBounce);
        }

        let omega = std::f64::consts::TAU / duration.as_secs_f64();
        let bounce = f64::from(bounce);
        let ratio = if bounce < 0.0 {
            (1.0 + bounce).recip()
        } else {
            1.0 - bounce
        };
        // Duration ranges from 1 ns to u64::MAX seconds; with f32 bounce in (-1, 1),
        // both results remain finite, positive f32 values across that entire range.
        Ok(Self {
            stiffness: (omega * omega) as f32,
            damping: (2.0 * omega * ratio) as f32,
        })
    }

    /// Spring stiffness for a framework's physical spring configuration.
    pub const fn stiffness(self) -> f32 {
        self.stiffness
    }

    /// Spring damping for a framework's physical spring configuration.
    pub const fn damping(self) -> f32 {
        self.damping
    }

    /// Unit mass used by the duration/bounce conversion.
    pub const fn mass(self) -> f32 {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_and_bounce_produce_expected_physical_motion() {
        for (bounce, preset, damping_ratio) in [
            (0.0, Spring::SMOOTH, 1.0),
            (0.15, Spring::SNAPPY, 0.85),
            (0.30, Spring::BOUNCY, 0.70),
        ] {
            let spring = Spring::new(Duration::from_millis(500), bounce).unwrap();
            assert!((spring.stiffness() - 157.91367).abs() < 0.0001);
            assert!((spring.damping() - preset.damping()).abs() < 0.0001);
            assert!(
                (spring.damping() / (2.0 * spring.stiffness().sqrt()) - damping_ratio).abs()
                    < 0.0001
            );
        }

        let slow = Spring::new(Duration::from_secs(1), -0.5).unwrap();
        let fast = Spring::new(Duration::from_millis(500), -0.5).unwrap();
        assert!((slow.damping() / (2.0 * slow.stiffness().sqrt()) - 2.0).abs() < 0.0001);
        assert!((fast.stiffness() / slow.stiffness() - 4.0).abs() < 0.0001);
        assert!((fast.damping() / slow.damping() - 2.0).abs() < 0.0001);
    }

    #[test]
    fn input_boundaries_never_produce_infinite_or_undamped_springs() {
        assert_eq!(
            Spring::new(Duration::ZERO, 0.0),
            Err(SpringError::ZeroDuration)
        );
        for bounce in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 1.0] {
            assert_eq!(
                Spring::new(Duration::from_secs(1), bounce),
                Err(SpringError::InvalidBounce)
            );
        }
        for duration in [Duration::from_nanos(1), Duration::MAX] {
            for bounce in [-1.0_f32.next_down(), 1.0_f32.next_down()] {
                let spring = Spring::new(duration, bounce).unwrap();
                assert!(spring.stiffness().is_finite() && spring.stiffness() > 0.0);
                assert!(spring.damping().is_finite() && spring.damping() > 0.0);
            }
        }
    }
}
