use gpui::{
    Bounds, ContentMask, DispatchPhase, ElementInputHandler, Entity, IntoElement, Pixels,
    TextAlign, TextRun, UnderlineStyle, Window, canvas, fill, point, prelude::*, px, size,
};

use super::TextInput;
use crate::theme::Theme;

pub(super) fn element(entity: Entity<TextInput>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            entity.update(cx, |input, cx| {
                if !input.disabled {
                    window.handle_input(
                        &input.focus_handle,
                        ElementInputHandler::new(bounds, cx.entity()),
                        cx,
                    );
                    let on_move = cx.listener(TextInput::mouse_move);
                    window.on_mouse_event(move |event, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            on_move(event, window, cx);
                        }
                    });
                }
                let theme = Theme::for_window(window);
                input.last_style = Some(window.text_style());
                input.last_bounds = Some(bounds);
                refresh_layout(input, window).expect("paint provides input bounds and text style");
                let line = input
                    .last_line
                    .as_ref()
                    .expect("refresh_layout shapes the current text");
                let caret = line.x_for_index(input.editing.cursor());
                let origin = point(bounds.left() - input.scroll_x, bounds.top());
                let focused = !input.disabled && input.focus_handle.is_focused(window);
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    if focused && !input.editing.selection.is_empty() {
                        let start = line.x_for_index(input.editing.selection.start);
                        let end = line.x_for_index(input.editing.selection.end);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(origin.x + start.min(end), origin.y),
                                size((end - start).abs(), bounds.size.height),
                            ),
                            theme.selection,
                        ));
                    }
                    line.paint(
                        origin,
                        bounds.size.height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .expect("a shaped input line must paint successfully");
                    // ponytail: A steady caret avoids idle frames; add focus-scoped blinking for native parity.
                    if focused && input.editing.selection.is_empty() && !input.read_only {
                        window.paint_quad(fill(
                            Bounds::<Pixels>::new(
                                point(origin.x + caret, origin.y),
                                size(px(1.0), bounds.size.height),
                            ),
                            theme.accent,
                        ));
                    }
                });
            });
        },
    )
    .w_full()
    .h(px(20.0))
}

pub(super) fn refresh_layout(input: &mut TextInput, window: &mut Window) -> Option<()> {
    let style = input.last_style.as_ref()?;
    let bounds = input.last_bounds?;
    let theme = Theme::for_window(window);
    let text = if input.editing.text.is_empty() {
        input.placeholder.clone()
    } else {
        input.editing.text.clone()
    };
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color: if input.editing.text.is_empty() {
            theme.secondary_foreground
        } else {
            style.color
        },
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let runs = if let Some(marked) = input.editing.marked.as_ref() {
        [
            TextRun {
                len: marked.start,
                ..run.clone()
            },
            TextRun {
                len: marked.len(),
                underline: Some(UnderlineStyle {
                    color: Some(run.color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..run.clone()
            },
            TextRun {
                len: text.len() - marked.end,
                ..run
            },
        ]
        .into_iter()
        .filter(|run| run.len > 0)
        .collect::<Vec<_>>()
    } else {
        vec![run]
    };
    // IME callbacks can query new text before the next paint; reuse the last rendered font and bounds.
    let line = window.text_system().shape_line(
        text,
        style.font_size.to_pixels(window.rem_size()),
        &runs,
        None,
    );
    let caret = line.x_for_index(input.editing.cursor());
    let width = (bounds.size.width - px(1.0)).max(px(0.0));
    input.scroll_x = input
        .scroll_x
        .clamp((caret - width).max(px(0.0)), caret.max(px(0.0)))
        .min((line.width - width).max(px(0.0)));
    input.last_line = Some(line);
    Some(())
}
