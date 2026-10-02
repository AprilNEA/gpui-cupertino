# Component contracts

Call `gpui_cupertino::init(cx)` once during application startup to register single-line editing keys. The application must bind Tab and Shift-Tab to `Window::focus_next` and `Window::focus_prev` in its root view. See `examples/components.rs` for a complete settings form.

`Theme::for_window(window)` resolves semantic colors from the current light or dark window appearance. Theme colors are library defaults. They do not read the system accent color. The palette checks require text contrast of at least 4.5:1 and control boundary contrast of at least 3:1 on the supported opaque surfaces.

## Button

See [Button and theme](button.md) for stable IDs, styles, keyboard activation, disabled behavior, focus, and accessibility. `track_focus` accepts a caller-owned focus handle. `aria_expanded` connects a trigger to the state of a popover.

## TextInput

Retain an `Entity<TextInput>`. Construct the input with an accessible label, the window, and its entity context. Render the entity as a child and use the parent layout to set its width.

The input supports grapheme movement and deletion, Shift selection, Home and End, selection by pointer, clipboard operations, undo and redo, and horizontal caret scrolling. Command shortcuts use Cmd on macOS and Ctrl on other targets. The GPUI platform input handler exposes UTF-16 selection, marked text, replacement ranges, and candidate bounds. An IME selection is relative to the inserted text. One composition forms one undo entry.

Tabs and line separators become spaces through the same editing path for typing, paste, accessibility updates, and programmatic values. The input emits `TextInputEvent::Changed` for user value changes, including composition updates. Enter emits `Submitted` when no composition is active. `set_value` clears history and moves the caret to the end without emitting `Changed`.

Read-only inputs retain focus, selection, copying, and submission. Disabled inputs reject edits and lose focus. Both states are exposed through the accessibility tree. Editable inputs expose a `SetValue` accessibility action.

The first implementation is a short, single-line field. Undo retains at most 100 full-value snapshots. The caret is steady. Password masking, word movement shortcuts, continuous pointer autoscroll, rich text, bidirectional visual cursor movement, and text-range accessibility navigation remain outside this milestone. Platform-protocol tests do not replace manual Chinese IME and VoiceOver acceptance in a native window.

## Popover

Retain one `Entity<PopoverState>` per rendered popover. Pass the state, trigger, accessible panel label, and styled content to `Popover::new`. The trigger must track `trigger_focus_handle`, expose `is_open` as its expanded state, and call `toggle` on activation.

The panel uses GPUI anchored layout and deferred painting. The panel fits the window and scrolls oversized content. The caller owns the content surface; `Glass` can supply the surface on supported macOS renderers. A scene regression checks that deferred glass samples preceding content and paints its foreground afterward.

Opening focuses the panel or the control passed to `initial_focus`. When focus is inside the panel, an unconsumed Escape event closes the panel. Clicking outside the panel and trigger closes the panel and preserves the outside click. Closing restores trigger focus only when the popover still owns focus. This nonmodal component does not trap Tab. Nested popovers require a shared dismissal policy and are not supported yet.

## Verification

Run `devenv test` before committing. The workspace checks compile the showcase and verify input editing, platform IME calls, pointer and keyboard dispatch, focus restoration, panel placement, accessibility trees and actions, and the existing Metal material regressions.

Run `cargo run -p gpui-cupertino --example components --locked` in `devenv shell` for native acceptance. Verify Chinese composition, candidate placement after horizontal scrolling, Tab traversal, Escape dismissal, light and dark appearance, and VoiceOver. Record native acceptance separately from simulated checks.
