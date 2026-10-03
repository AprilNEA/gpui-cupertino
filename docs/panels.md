# Menus and panels

Call `gpui_cupertino::init(cx)` once before creating windows. Startup registers panel dismissal, modal Tab navigation, menu navigation, and tooltip dismissal. Application Tab bindings remain responsible for ordinary nonmodal controls.

## Shared panel state

Retain one `Entity<PanelState>` per panel. `PopoverState` is a compatibility alias for `PanelState`. Give the opening control `trigger_focus_handle()` and report `is_open()` through the control's expanded accessibility state. Use `open`, `close`, and `toggle` for all state changes. `initial_focus` accepts a descendant control's focus handle.

Panels form an opening-order stack within each window. The top panel handles Escape and outside dismissal. One input event closes at most one panel. Closing an ancestor programmatically closes all panels opened above that ancestor. Keep each open panel rendered until the panel closes. Retain one state entity for exactly one rendered panel.

A popover permits an outside click to reach its destination. A popover closes without restoring trigger focus when another control already owns focus. Nested panels use GPUI deferred painting with stack-depth priorities, so child panels paint above their parents. Content remains caller-owned. Panels fit the viewport and scroll oversized content. Newly focused child controls scroll into view with minimum displacement on the next frame. Wheel input keeps control of the offset until focus changes again.

## Menus

Create `MenuState` from `MenuItem::new(id, label)` values. IDs must be unique. Use `disabled(true)` to exclude an action from selection. Retain the state entity and subscribe to `MenuEvent`; the event supplies the selected item's ID. Render `Menu::new(&state, trigger, label)` and connect the trigger to the menu state's focus handle, expanded state, and `toggle` method.

Opening focuses the first enabled item. Up and Down wrap through enabled items. Home and End choose the first and last enabled items. Hover moves focus to an enabled item. Enter, Space, pointer clicks, and accessibility activation use the same selection path. Selection closes the menu and restores trigger focus. Tab and Shift-Tab close the menu and continue control traversal, including empty menus and menus containing only disabled items. Empty menus and menus containing only disabled items retain panel focus and can be dismissed.

## Dialogs and sheets

Wrap the full window content in `ModalHost::new(background)`. Add a `Dialog::new(&state, label, content)` or `Sheet::new(&state, label, content)`. The panel appears only while its retained state is open. Dialogs are centered. Sheets attach to `SheetEdge::Top`, `Trailing`, or `Bottom`. Both components have a modal dialog accessibility role.

The host paints the background through `gpui::inert`. The inert subtree does not register pointer handlers, keyboard handlers, actions, focus targets, platform text-input handlers, or accessibility nodes and actions. Deferred descendants retain the boundary. Cached views rebuild their registrations when the boundary changes. The modal backdrop consumes outside pointer input. Outside clicks do not dismiss by default; `dismiss_on_outside_click(true)` enables dismissal without forwarding the click.

Tab and Shift-Tab cycle through the active modal subtree. Escape closes the top panel. Closing restores focus to the opening control. A nested `ModalHost` can make an existing dialog inert while a second dialog or sheet is open.

Put background action handlers inside the host's background subtree. Application-global actions and persistent application subscriptions are outside the inert boundary. The application must gate background operations registered through those APIs. Programmatic focus requests do not grant an inert subtree any input handlers. The host restores modal focus on its next render.

```rust,ignore
let trigger = Button::new("edit", "Edit")
    .track_focus(state.read(cx).trigger_focus_handle())
    .aria_expanded(state.read(cx).is_open());

ModalHost::new(background.child(trigger))
    .dialog(Dialog::new(&state, "Edit profile", editor))
```

## Tooltips

Create `TooltipState::new(description, &trigger_focus, window, cx)` and retain the entity. Render `Tooltip::new(&state, trigger)`. The trigger must use the observed focus handle. Tooltip content never takes focus or handles pointer input.

Hover displays the hint after 500 ms. Keyboard focus displays the hint immediately. Losing both hover and focus hides the hint. Escape or trigger activation hides the hint until hover and focus have both left. The wrapper exposes an accessibility description and tooltip property even when the visual hint is hidden. The visible hint has the tooltip role. A trigger with a separate accessibility description may also use `TooltipState::description()`.

Escape dismisses a visible hover hint even when another control owns keyboard focus. The dismissal precedes parent key handlers and does not move focus. Pointer movement within the trigger does not reopen the dismissed hint. Inert or unmounted hints do not intercept keys. Explicit application key bindings retain their precedence.

## Verification and scope

Run the project-detail composition with `devenv shell -- cargo run -p gpui-cupertino --example detail`. The toolbar combines a focus tooltip and an action menu. The trailing sheet edits retained text-input drafts. Save updates the project overview. Cancel opens a confirmation dialog when the draft differs from the saved project. Keep editing or Escape dismisses only that confirmation and restores the sheet's Cancel control. Discard closes both panels, restores the saved values, and returns focus to Edit details. Escape from the sheet also discards its temporary draft. Data remains in memory for the window's lifetime.

The example scopes application Tab bindings under the ancestor `DetailShowcase` key context. Menu and modal key contexts retain ownership of their navigation keys. Context-free bindings registered later are explicit application overrides.

Run `devenv shell -- cargo test -p gpui-cupertino --example detail --locked` for the composition test. The test uses simulated accessibility and keyboard dispatch to exercise menu opening, text editing, saving, modal Tab cycling, nested dismissal, draft rollback, background exclusion, and focus restoration.

The component tests simulate nested dismissal, descendant closing, menu navigation, disabled accessibility actions, modal Tab cycles, background action exclusion, stale accessibility requests, tooltip lifetime, and deferred inert content. Existing popover placement, focus, accessibility, and material scene regressions remain required.

The panels use semantic opaque surfaces. Native VoiceOver operation, screen appearance, animation parity, and active Clear or Regular material matching require separate native acceptance. Passing simulated dispatch checks does not establish native visual parity.
