# Choice controls

`Toggle`, `Checkbox`, and `RadioGroup` are controlled components. Keep each element ID stable across renders. Store values in the parent view. Update the parent value in `on_change` and call `cx.notify()`. If the parent rejects a proposed value, the displayed value remains unchanged.

```rust
use gpui_cupertino::components::{Checkbox, CheckState, ChoiceOption, RadioGroup, Toggle};

let wireless = Toggle::new("wireless", "Wireless", true);
let folders = Checkbox::new("folders", "All folders", CheckState::Mixed);
let destination = RadioGroup::new("destination", "Destination", [
    ChoiceOption::new("local", "On this Mac"),
    ChoiceOption::new("cloud", "Cloud").disabled(true),
]).selected("local");
```

## Toggle and Checkbox

`Toggle::on_change` receives `&bool`. `Checkbox::on_change` receives `&CheckState`. Activating an unchecked or mixed checkbox proposes `CheckState::Checked`. Activating a checked checkbox proposes `CheckState::Unchecked`. A checkbox also accepts a Boolean initial value.

The visible label is the accessible name. Pointer release, Space release, Enter release, and the accessibility Click action use the same callback. Modified Space and Enter do not activate the control. `track_focus` accepts a caller-owned focus handle. The application owns Tab and Shift-Tab navigation, as for `Button`.

`disabled(true)` removes the tab stop, exposes the disabled state, and rejects pointer, keyboard, and accessibility activation. Disabling a control cancels a pending press. Both components implement `Styled` for layout refinements.

The switch thumb uses the existing GPUI spring and the Cupertino `SNAPPY` parameters. A stable element ID preserves position and velocity across reversals. A newly mounted switch starts at the supplied value. GPUI reduced motion completes an active transition immediately and prevents further transition frames. The track color changes with the supplied value. These motion parameters are library defaults; native switch motion parity has not been measured.

## RadioGroup

Each `ChoiceOption` has a stable string ID and a visible label. IDs must be unique within the group; duplicate IDs panic during construction. Reordering options preserves the focused option's identity. `selected(id)` reports the parent's value. `on_change` receives `&SharedString` only when activation proposes a different ID.

The group exposes a `RadioGroup` role. Each option exposes a `RadioButton` role, accessible name, checked value, and disabled state. Only one enabled option is a Tab stop: the selected option, or the first enabled option when the selected value is absent, unknown, or disabled. The group has no Tab stop when all options are disabled or the group is empty.

Arrow keys select and focus the next or previous enabled option, with wrapping. Home and End select the first and last enabled options. Modified navigation keys are left to the application. Pointer and accessibility activation focus and select the target option. Re-activating the selected option does not emit a duplicate change.

When a focused option is removed or disabled, focus moves to the selected enabled option or the first enabled option. If no enabled option remains, the group releases focus. This focus repair does not change the parent's value or emit a selection callback.

## Verification

Run `cargo test -p gpui-cupertino --test choice_controls --locked` in the existing devenv shell. The tests dispatch platform pointer and keyboard events, invoke accessibility actions, inspect accessibility state and focus, exercise option reordering and disabling, and verify switch frame scheduling with reduced motion.

These CPU tests verify the GPUI contracts. Native VoiceOver, pointer feel, light and dark visual review, and platform motion matching still need native acceptance. No material parity claim follows from the choice control tests.
