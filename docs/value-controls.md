# Value controls

Call `gpui_cupertino::init(cx)` once before creating application windows to register component key bindings. `Slider` and `Stepper` are controlled components. Create one `ValueRange`, render the parent's value, and store each `on_change` request in the parent. Keep element IDs stable across renders. Both components support caller-owned focus handles and the existing application Tab bindings.

```rust
use gpui_cupertino::components::{Slider, ValueRange, ValueRangeError};

fn volume_slider() -> Result<Slider, ValueRangeError> {
    let range = ValueRange::new(0.0, 100.0, 5.0)?;
    Ok(Slider::new("volume", "Volume", range)
        .value(50.0)?
        .on_change(|value, _, _| println!("Requested volume: {value}")))
}
```

`ValueRange::new` rejects nonfinite bounds, unordered bounds, nonpositive steps, steps larger than the interval, and steps that cannot change the endpoints at `f64` precision. The interval must contain at most `2^52` steps. `.value` rejects nonfinite values. Finite values are clamped and snapped to the nearest step or endpoint. Steps originate at the minimum. The maximum remains reachable when the width is not divisible by the step.

Arrow keys increment or decrement one step. Home and End select the endpoints. Modified arrow keys remain available to the application. Accessibility exposes the numeric value, bounds, step, and increment, decrement, and numeric `SetValue` actions. Invalid external numeric values do not change the control. A control without `on_change` is read-only. Disabled controls reject input and do not register a tab stop.

The slider uses a horizontal track. Pointer presses update the value and start a drag. The drag continues outside the slider, clamps at the endpoints, and ends on pointer release. Disabling the slider, removing its change listener, or moving focus cancels the drag. The stepper has one keyboard tab stop. Its decrease and increase buttons expose separate accessible labels and disable at their respective boundaries. Numeric text uses Rust's standard numeric display; locale-specific formatting is not part of this API.

## Segmented selection

`SegmentedControl::new(id, label, options)` accepts `ChoiceOption` values shared with `RadioGroup`. Use `.selected(option_id)` and `.on_change` to retain selection in the parent. Option IDs must remain stable and unique. Use `.disabled(true)` on an option or the whole control to reject selection.

The control uses radio semantics because selecting a value does not require a tab panel. Left and Right wrap through enabled options. Home and End select the first or last enabled option. The selected enabled option is the group's single tab stop. If no enabled option is selected, the first enabled option is the tab stop. Accessibility reports each option's checked state.

## Progress and loading

`Progress::new(id, label)` creates an indeterminate loading indicator. `.value(fraction)` enables determinate progress and rejects values outside the finite inclusive interval `0..=1`. Accessibility reports a progress indicator, a numeric value only for determinate progress, and a busy state until the fraction reaches one.

The loading segment requests frames only during visible painting. A clipped, removed, or nonvisible-window indicator stops frame requests. Reduced motion displays a stationary segment and schedules no animation. Determinate progress does not schedule animation. Remove indicators when a loading operation ends; do not hide a running indicator with opacity alone.

## Verification

Run `cargo test -p gpui-cupertino --test value_controls --lib --locked` in the existing devenv shell. Tests exercise actual GPUI pointer, key, focus, and accessibility dispatch; range boundaries and invalid values; shared segmented navigation; and loading frame cancellation. Simulated checks do not establish native VoiceOver or macOS visual parity.
