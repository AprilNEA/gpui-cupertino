use gpui::{App, ClipboardItem, Context, Div, KeyBinding, Stateful, actions, prelude::*};

use super::{TextInput, TextInputEvent};

actions!(
    cupertino_input,
    [
        Left,
        Right,
        SelectLeft,
        SelectRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Backspace,
        Delete,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Submit,
    ]
);

pub(crate) fn init(cx: &mut App) {
    let context = Some("CupertinoTextInput");
    cx.bind_keys([
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("shift-home", SelectHome, context),
        KeyBinding::new("shift-end", SelectEnd, context),
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("enter", Submit, context),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("cmd-shift-left", SelectHome, context),
        KeyBinding::new("cmd-shift-right", SelectEnd, context),
    ]);
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new(&format!("{modifier}-a"), SelectAll, context),
        KeyBinding::new(&format!("{modifier}-c"), Copy, context),
        KeyBinding::new(&format!("{modifier}-x"), Cut, context),
        KeyBinding::new(&format!("{modifier}-v"), Paste, context),
        KeyBinding::new(&format!("{modifier}-z"), Undo, context),
        KeyBinding::new(&format!("{modifier}-shift-z"), Redo, context),
    ]);
}

pub(super) fn handlers(field: Stateful<Div>, cx: &mut Context<TextInput>) -> Stateful<Div> {
    field
        .on_action(cx.listener(|input, _: &Left, _, cx| input.move_cursor(false, false, cx)))
        .on_action(cx.listener(|input, _: &Right, _, cx| input.move_cursor(true, false, cx)))
        .on_action(cx.listener(|input, _: &SelectLeft, _, cx| input.move_cursor(false, true, cx)))
        .on_action(cx.listener(|input, _: &SelectRight, _, cx| input.move_cursor(true, true, cx)))
        .on_action(cx.listener(|input, _: &Home, _, cx| input.move_to_edge(false, false, cx)))
        .on_action(cx.listener(|input, _: &End, _, cx| input.move_to_edge(true, false, cx)))
        .on_action(cx.listener(|input, _: &SelectHome, _, cx| input.move_to_edge(false, true, cx)))
        .on_action(cx.listener(|input, _: &SelectEnd, _, cx| input.move_to_edge(true, true, cx)))
        .on_action(cx.listener(|input, _: &SelectAll, _, cx| input.select_all(cx)))
        .on_action(cx.listener(|input, _: &Backspace, _, cx| input.delete(false, cx)))
        .on_action(cx.listener(|input, _: &Delete, _, cx| input.delete(true, cx)))
        .on_action(cx.listener(|input, _: &Copy, _, cx| copy(input, cx)))
        .on_action(cx.listener(|input, _: &Cut, _, cx| {
            if input.editable() && !input.editing.selection.is_empty() {
                copy(input, cx);
                input.edit(|editing| editing.replace(None, ""), cx);
            }
        }))
        .on_action(cx.listener(|input, _: &Paste, _, cx| {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                input.edit(|editing| editing.replace(None, &text), cx);
            }
        }))
        .on_action(cx.listener(|input, _: &Undo, _, cx| input.edit(|editing| editing.undo(), cx)))
        .on_action(cx.listener(|input, _: &Redo, _, cx| input.edit(|editing| editing.redo(), cx)))
        .on_action(cx.listener(|input, _: &Submit, _, cx| {
            if input.editing.marked.is_none() {
                cx.emit(TextInputEvent::Submitted(input.editing.text.clone()));
            }
        }))
}

fn copy(input: &TextInput, cx: &mut Context<TextInput>) {
    if !input.editing.selection.is_empty() {
        cx.write_to_clipboard(ClipboardItem::new_string(
            input.editing.text[input.editing.selection.clone()].to_owned(),
        ));
    }
}
