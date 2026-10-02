# Button and theme

`Button` is a labeled GPUI component. The ordinary style uses the control background; `.primary()` uses the accent background. The palette follows the window's light or dark appearance on each render.

```rust
use gpui_cupertino::components::Button;

let save = Button::new("save", "Save")
    .primary()
    .disabled(!can_save)
    .on_click(cx.listener(|view, _, window, cx| {
        view.save(window, cx);
    }));
```

Keep the ID stable and unique within its parent. The visible label supplies the accessible name. Use `gpui::Styled` methods for layout overrides.

An enabled button accepts a primary pointer click and an unmodified Enter or Space press followed by release. GPUI cancels keyboard activation if focus changes during the press. `.primary()` changes appearance; the application still owns window-wide default actions.

Bind Tab and Shift-Tab to GPUI's `Window::focus_next` and `Window::focus_prev` at application startup. Enabled buttons are tab stops. `.track_focus(&handle)` allows programmatic focus and preserves the handle's tab index. Keyboard navigation shows a focus ring without changing layout.

`.disabled(true)` removes input listeners and the tab stop, dims the control, and exposes the disabled state to assistive technology. A disabled button retains its role and name but exposes no Click or Focus action. `.aria_expanded(open)` reports the state of a popup controlled by the button.

`Theme::for_window(window)` exposes semantic colors for application backgrounds, text, control backgrounds and borders, accents, focus rings, and text selection. `Theme::from(WindowAppearance)` resolves the same palette without a window. These are opaque Cupertino defaults with readable text contrast, not native AppKit controls or a system accent-color observer.

Run `cargo test -p gpui-cupertino --test button --test accessibility` in the configured `devenv` shell to verify pointer, keyboard, focus, disabled-state transitions, and accessibility dispatch. The theme unit test checks text and boundary contrast in all four GPUI appearances.
