# Forms and scrolling

`Form` groups controls vertically and exposes a named form. `FormSection` adds a heading, an opaque surface, and optional help text. Child controls retain their own input, focus, and validation state.

```rust
use gpui::prelude::*;
use gpui_cupertino::components::{Button, Form, FormField, FormSection};

let form = Form::new("preferences", "Preferences").child(
    FormSection::new("account", "Account")
        .description("Settings are kept locally.")
        .child(FormField::new("profile", "Profile", Button::new("edit", "Edit profile"))),
);
```

`FormField::new(id, label, control)` places a visible label above its control. Give the control its own accessible name. The field exposes a named group; `description` supplies visible help and the group's accessible description. Use `error` for a caller-owned validation message. The error becomes an assertive accessibility alert with the message as its value. IDs must remain stable, including when the error appears or disappears.

`Toolbar` arranges caller-supplied controls horizontally and wraps when space is limited. The toolbar exposes a name and toolbar role. Its controls retain their normal Tab stops. `EmptyState::new(id, title, description)` displays a title, explanation, and optional action children. The empty state exposes a named group and accessible description. Both support normal GPUI styling.

`ScrollArea` provides a bounded viewport. Set its height or constrain its size through flex layout. The default axis is vertical; `ScrollAxes::Horizontal` and `ScrollAxes::Both` enable the other configurations. GPUI handles wheel input and clipping. A stable element ID retains the scroll offset. Pass a retained `ScrollHandle` to `track_scroll` when the application must observe the position or reveal a child programmatically.

When the area itself has focus, arrow keys scroll by 40 logical pixels, Page Up/Down move by 90% of the viewport, and Home/End reach the primary-axis boundary. Child controls keep their own key handling. Accessibility scroll actions use the same page movement and clamping. A smaller content extent clamps the existing offset to the new boundary. Changing the permitted axes resets the excluded axis to zero and preserves the permitted offset.

ScrollArea mounts all supplied children. Use the planned virtual collection component for large data sets. These containers use the shared theme and GPUI layout; native macOS appearance parity is not implied by the API.

Run `devenv shell -- cargo test -p gpui-cupertino --test layout --locked` to verify keyboard and accessibility scrolling, range shrinkage, child input ownership, and validation-alert removal.
