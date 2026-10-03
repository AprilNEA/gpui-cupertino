//! CPU-only timings for virtual collection layout, filtering, and sorting.

use std::time::Instant;

use gpui::{
    Focusable, InputEvent, KeyDownEvent, Keystroke, TestAppContext, TestDispatcher, px, size,
};
use gpui_cupertino::components::{
    CollectionRow, CollectionView, ReorderDirection, SelectionMode, TableColumn,
};

fn main() {
    println!("CPU TestAppContext; no native window, Metal renderer, or screen capture");
    println!(
        "profile={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "optimized"
        }
    );
    for count in [1_000, 10_000, 50_000] {
        let mut cx = TestAppContext::build(TestDispatcher::new(0), Some("collections_benchmark"));
        cx.update(gpui_cupertino::init);
        let window = cx.open_window(size(px(640.0), px(480.0)), |_, cx| {
            CollectionView::new(
                "Records",
                (0..count).map(|index| {
                    CollectionRow::new(
                        format!("id-{index}"),
                        format!(
                            "{} Record {index:05}",
                            if index < count / 2 { "First" } else { "Second" }
                        ),
                    )
                    .cell(format!("{}", (index * 17) % 31))
                }),
                cx,
            )
            .expect("generated IDs are unique")
            .selection_mode(SelectionMode::Multiple)
            .with_columns([
                TableColumn::new("name", "Name", px(260.0)),
                TableColumn::new("value", "Value", px(180.0)),
            ])
            .expect("generated rows have two cells and valid columns")
        });
        cx.run_until_parked();
        for operation in ["layout", "filter", "sort", "selected-layout", "reorder"] {
            if matches!(operation, "selected-layout" | "reorder") {
                window
                    .update(&mut cx, |view, window, cx| {
                        view.clear_sort(cx);
                        view.set_filter("First", cx);
                        view.focus_handle(cx).focus(window, cx);
                    })
                    .expect("the benchmark window remains open");
                cx.run_until_parked();
                cx.test_window(window.into()).simulate_input(
                    KeyDownEvent {
                        keystroke: Keystroke::parse(if cfg!(target_os = "macos") {
                            "cmd-a"
                        } else {
                            "ctrl-a"
                        })
                        .expect("the platform shortcut is valid"),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                );
                cx.run_until_parked();
                window
                    .update(&mut cx, |view, _, cx| {
                        assert_eq!(view.selected_ids().len(), count / 2);
                        view.set_filter("", cx);
                    })
                    .expect("the benchmark window remains open");
                cx.run_until_parked();
            }
            let mut times = Vec::with_capacity(31);
            for iteration in 0..34 {
                let start = Instant::now();
                window
                    .update(&mut cx, |view, _, cx| match operation {
                        "layout" | "selected-layout" => cx.notify(),
                        "filter" => view.set_filter(if iteration % 2 == 0 { "9" } else { "" }, cx),
                        "sort" => view
                            .toggle_sort("value", cx)
                            .expect("the value column exists"),
                        "reorder" => assert!(view.move_selected(
                            if iteration % 2 == 0 {
                                ReorderDirection::Down
                            } else {
                                ReorderDirection::Up
                            },
                            cx
                        )),
                        _ => unreachable!(),
                    })
                    .expect("the benchmark window remains open");
                cx.run_until_parked();
                if iteration >= 3 {
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            times.sort_by(f64::total_cmp);
            println!(
                "rows={count}, operation={operation}, median_ms={:.3}, p95_ms={:.3}",
                times[15], times[29]
            );
        }
    }
}
