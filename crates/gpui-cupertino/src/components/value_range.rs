use std::{error::Error, fmt};

/// A finite numeric interval with steps measured from its minimum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    min: f64,
    max: f64,
    step: f64,
}

/// Invalid bounds, step size, or a nonfinite control value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueRangeError {
    /// The bounds or step do not form a finite, representable interval.
    InvalidRange,
    /// A control value is not finite.
    NonFiniteValue,
    /// Fractional bounds extend outside the inclusive interval `0..=1`.
    FractionBounds,
}

impl fmt::Display for ValueRangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRange => "require finite ordered bounds and a representable positive step",
            Self::NonFiniteValue => "control value must be finite",
            Self::FractionBounds => "fractional bounds must be between zero and one",
        })
    }
}

impl Error for ValueRangeError {}

impl ValueRange {
    /// Validates the bounds and step size.
    ///
    /// The interval must have positive finite width. The step must not exceed
    /// that width and must change both endpoints at `f64` precision.
    pub fn new(min: f64, max: f64, step: f64) -> Result<Self, ValueRangeError> {
        let width = max - min;
        if !min.is_finite()
            || !max.is_finite()
            || !width.is_finite()
            || width <= 0.0
            || !step.is_finite()
            || step <= 0.0
            || step > width
            || min + step == min
            || max - step == max
            || width / step > (1_u64 << 52) as f64
        {
            return Err(ValueRangeError::InvalidRange);
        }
        Ok(Self { min, max, step })
    }

    /// Returns the lower endpoint.
    pub fn minimum(self) -> f64 {
        self.min
    }

    /// Returns the upper endpoint.
    pub fn maximum(self) -> f64 {
        self.max
    }

    /// Returns the spacing between interior values.
    pub fn step(self) -> f64 {
        self.step
    }

    /// Clamps and snaps a finite value to the nearest step or endpoint.
    ///
    /// The upper endpoint remains reachable when the width is not divisible
    /// by the step. Nonfinite values are rejected.
    pub fn snap(self, value: f64) -> Result<f64, ValueRangeError> {
        if !value.is_finite() {
            return Err(ValueRangeError::NonFiniteValue);
        }
        let value = value.clamp(self.min, self.max);
        let stepped = (((value - self.min) / self.step).round() * self.step + self.min)
            .clamp(self.min, self.max);
        Ok(if self.max - value < (stepped - value).abs() {
            self.max
        } else {
            stepped
        })
    }

    pub(super) fn increment(self, value: f64) -> f64 {
        let index = ((value - self.min) / self.step).round();
        ((index + 1.0) * self.step + self.min).min(self.max)
    }

    pub(super) fn decrement(self, value: f64) -> f64 {
        let index = ((value - self.min) / self.step).round();
        let previous = if value == self.max && self.min + index * self.step < self.max {
            index
        } else {
            index - 1.0
        };
        (previous * self.step + self.min).max(self.min)
    }

    pub(super) fn fraction(self, value: f64) -> f32 {
        ((value - self.min) / (self.max - self.min)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_rejects_invalid_inputs_and_preserves_both_endpoints() {
        for args in [
            (0.0, 0.0, 1.0),
            (1.0, 0.0, 1.0),
            (0.0, 1.0, 0.0),
            (0.0, 1.0, 2.0),
            (0.0, f64::INFINITY, 1.0),
            (-f64::MAX, f64::MAX, 1.0),
            (1e20, 2e20, 1.0),
            (0.0, 1.0, f64::NAN),
        ] {
            assert!(ValueRange::new(args.0, args.1, args.2).is_err());
        }
        let range = ValueRange::new(-1.0, 1.0, 0.3).unwrap();
        assert!(range.snap(f64::NAN).is_err());
        assert_eq!(range.snap(-3.0), Ok(-1.0));
        assert_eq!(range.snap(3.0), Ok(1.0));
        assert_eq!(range.increment(0.8), 1.0);
        assert!((range.decrement(1.0) - 0.8).abs() < 1e-12);
        assert_eq!(range.decrement(-1.0), -1.0);
        let tenths = ValueRange::new(0.0, 1.0, 0.1).unwrap();
        let mut value = 0.0;
        for _ in 0..10 {
            value = tenths.increment(value);
        }
        assert_eq!(value, 1.0);
    }
}
