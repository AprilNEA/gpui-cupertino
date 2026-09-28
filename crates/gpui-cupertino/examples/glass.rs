//! Run with `cargo run -p gpui-cupertino --example glass` on macOS.
//! Add `-- --inspect-color-space` to print the attached Metal layer's color-space state and exit.

#[cfg(target_os = "macos")]
#[path = "glass/diagnostics.rs"]
mod diagnostics;

#[cfg(target_os = "macos")]
mod demo {
    use gpui::{
        AnimationExt, App, Bounds, Context, IntoElement, Render, Role, Toggled, Window,
        WindowAppearance, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
    };
    use gpui_cupertino::{
        cupertino::{
            materials::{GlassMaterial, GlassMaterialOptions, GlassShape, Refraction},
            motion::Spring,
        },
        materials::{Glass, GlassAccessibility},
        motion::spring,
        platform::accessibility_preferences,
    };

    #[derive(Clone, Copy)]
    enum Control {
        Dark,
        Transparency,
        Contrast,
        Motion,
        Identity,
        Reverse,
    }

    #[derive(Default)]
    struct Demo {
        accessibility: GlassAccessibility,
        identity: bool,
        expanded: bool,
        reduce_motion: bool,
    }

    fn is_dark(window: &Window) -> bool {
        matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        )
    }

    impl Demo {
        fn toggle(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
            match control {
                Control::Dark => cx.set_window_appearance(Some(if is_dark(window) {
                    WindowAppearance::Light
                } else {
                    WindowAppearance::Dark
                })),
                Control::Transparency => {
                    self.accessibility.reduce_transparency =
                        !self.accessibility.reduce_transparency;
                }
                Control::Contrast => {
                    self.accessibility.increase_contrast = !self.accessibility.increase_contrast;
                }
                Control::Motion => {
                    self.reduce_motion = !self.reduce_motion;
                    let system = accessibility_preferences(cx);
                    cx.set_reduce_motion(self.reduce_motion || system.reduce_motion);
                }
                Control::Identity => self.identity = !self.identity,
                Control::Reverse => self.expanded = !self.expanded,
            }
            cx.notify();
        }

        fn control(
            &self,
            control: Control,
            label: &'static str,
            selected: bool,
            cx: &Context<Self>,
        ) -> impl IntoElement {
            div()
                .id(label)
                .tab_index(0)
                .role(Role::Button)
                .aria_label(label)
                .aria_toggled(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .cursor_pointer()
                .px_3()
                .py_2()
                .rounded_lg()
                .border_1()
                .border_color(rgb(if selected { 0x3182ee } else { 0x808080 }))
                .focus_visible(|style| style.border_color(rgb(0x3182ee)).border_2())
                .hover(|style| style.bg(rgb(0x808080).opacity(0.18)))
                .child(format!("{} {label}", if selected { "●" } else { "○" }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toggle(control, window, cx);
                }))
        }

        fn material(&self, shape: GlassShape, dark: bool) -> GlassMaterial {
            // These local constants satisfy GlassMaterial's validation contract.
            GlassMaterial::try_from(if self.identity {
                GlassMaterialOptions {
                    shape,
                    ..Default::default()
                }
            } else {
                GlassMaterialOptions {
                    shape,
                    blur_sigma: 8.0,
                    tint: if dark {
                        [0.012, 0.018, 0.035, 0.55]
                    } else {
                        [1.0, 1.0, 1.0, 0.5]
                    },
                    saturation: 1.25,
                    refraction: Refraction {
                        amount: 7.0,
                        width: 18.0,
                        direction_mix: 0.25,
                    },
                    dispersion: 0.8,
                    highlight: 0.35,
                    ..Default::default()
                }
            })
            .expect("the demo uses finite, in-range material constants")
        }
    }

    impl Render for Demo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let system = accessibility_preferences(cx);
            cx.set_reduce_motion(self.reduce_motion || system.reduce_motion);
            let dark = is_dark(window);
            let foreground = rgb(if dark { 0xffffff } else { 0x151525 });
            let material = self.material(
                GlassShape::RoundedRectangle {
                    corner_radius: 22.0,
                },
                dark,
            );
            let checkerboard = div()
                .w(px(1280.0))
                .flex()
                .flex_col()
                .children((0..40).map(|y| {
                    div()
                        .flex()
                        .h(px(64.0))
                        .flex_shrink_0()
                        .children((0..20).map(move |x| {
                            let color = match ((x + y) % 2 == 0, dark) {
                                (true, true) => 0x152946,
                                (false, true) => 0x844549,
                                (true, false) => 0x76bced,
                                (false, false) => 0xf1a684,
                            };
                            div()
                                .size(px(64.0))
                                .flex_shrink_0()
                                .bg(rgb(color))
                                .p_1()
                                .text_xs()
                                .child(format!("{x}, {y}"))
                        }))
                }));

            div().size_full().flex().flex_col().text_color(foreground)
                .bg(rgb(if dark { 0x15151a } else { 0xf5f5f7 }))
                .child(div().p_4().flex().flex_col().gap_2()
                    .child("Cupertino · window-local glass")
                    .child(div().flex().flex_wrap().gap_2().text_sm()
                        .child(self.control(Control::Dark, "Dark", dark, cx))
                        .child(self.control(Control::Transparency, "Force opaque", self.accessibility.reduce_transparency, cx))
                        .child(self.control(Control::Contrast, "Force contrast", self.accessibility.increase_contrast, cx))
                        .child(self.control(Control::Motion, "Force reduced motion", self.reduce_motion, cx))
                        .child(self.control(Control::Identity, "Identity capture", self.identity, cx)))
                    .child(div().text_xs().child(format!(
                        "Scale {:.1}× · System: opaque {}, contrast {}, reduced motion {} · Scroll and resize to inspect",
                        window.scale_factor(), system.reduce_transparency, system.increase_contrast, system.reduce_motion
                    ))))
                .child(div().relative().flex_1().min_h_0().overflow_hidden()
                    .child(div().id("coordinate-grid").size_full().overflow_scroll().child(checkerboard))
                    .child(div().absolute().top(px(36.0)).left(px(24.0)).right(px(24.0))
                        .child(Glass::new(material,
                            div().p_4().flex().flex_col().gap_2()
                                .child("These labels are drawn AFTER the background capture.")
                                .child(div().flex().items_center().gap_4()
                                    .child(self.control(Control::Reverse, "Reverse spring", self.expanded, cx))
                                    .child(div().relative().w(px(200.0)).h(px(28.0))
                                        .child(div().absolute().size(px(24.0)).rounded_full().bg(rgb(0x3182ee))
                                            .with_spring("position", spring(Spring::SNAPPY)
                                                .to(px(if self.expanded { 172.0 } else { 0.0 })).with_epsilon(0.05),
                                                |element, position| element.left(position)))))
                                .child(div().text_xs().child("Click rapidly: position and velocity continue across reversals.")))
                            .accessibility(self.accessibility)))
                    .child(div().absolute().top(px(240.0)).left(px(24.0)).flex().gap_2().items_center()
                        .child(Glass::new(self.material(GlassShape::Capsule, dark),
                            div().h(px(72.0)).flex().items_center().justify_center().text_sm()
                                .child("Independent capsule")
                                .with_spring("capsule-width", spring(Spring::BOUNCY)
                                    .to(px(if self.expanded { 320.0 } else { 180.0 })).with_epsilon(0.05),
                                    |element, width| element.w(width)))
                            .accessibility(self.accessibility))
                        .child(Glass::new(self.material(GlassShape::Circle, dark),
                            div().size(px(96.0)).flex().items_center().justify_center().text_sm().child("Circle"))
                            .accessibility(self.accessibility)))
                    .child(div().absolute().bottom(px(24.0)).left(px(24.0))
                        .child(Glass::new(material,
                            div().p_4().text_sm().child("Neighboring glass stays separate; this demo does not fuse shapes."))
                            .accessibility(self.accessibility))))
        }
    }

    pub fn run() {
        let inspect_color_space = std::env::args().any(|arg| arg == "--inspect-color-space");
        gpui_platform::application().run(move |cx: &mut App| {
            accessibility_preferences(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(960.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Cupertino Glass");
                    cx.new(|_| Demo::default())
                },
            )
            .expect("the glass demo requires a graphical macOS session");
            cx.activate(true);
            if inspect_color_space {
                super::diagnostics::inspect_color_space_and_quit(cx);
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() {
    demo::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The glass example requires macOS and the Metal renderer.");
    std::process::exit(1);
}
