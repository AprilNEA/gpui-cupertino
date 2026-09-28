use super::*;
use crate::platform::{AccessibilityPreferences, set_test_preferences};
use cupertino::materials::GlassMaterialOptions;
use gpui::{
    Backdrop, Context, Corners, Edges, Quad, Render, ScaledPixels, TestAppContext, WindowHandle,
    div, prelude::*,
};

struct Probe {
    shape: GlassShape,
    accessibility: GlassAccessibility,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let material = GlassMaterial::try_from(GlassMaterialOptions {
            shape: self.shape,
            blur_sigma: 3.0,
            ..Default::default()
        })
        .unwrap();
        div().size_full().bg(rgb(0x224466)).child(
            div()
                .absolute()
                .left(px(10.0))
                .top(px(8.0))
                .w(px(90.0))
                .h(px(32.0))
                .overflow_hidden()
                .opacity(0.4)
                .child(
                    Glass::new(
                        material,
                        div().relative().w(px(100.0)).h(px(40.0)).child(
                            div()
                                .absolute()
                                .left(px(52.0))
                                .top(px(12.0))
                                .w(px(1.0))
                                .h(px(16.0))
                                .bg(rgb(0xffffff)),
                        ),
                    )
                    .accessibility(self.accessibility),
                ),
        )
    }
}

fn open(cx: &mut TestAppContext, shape: GlassShape) -> WindowHandle<Probe> {
    cx.update(|cx| set_test_preferences(cx, AccessibilityPreferences::default()));
    let window = cx.open_window(size(px(160.0), px(80.0)), move |_, _| Probe {
        shape,
        accessibility: GlassAccessibility::default(),
    });
    cx.run_until_parked();
    window
}

fn painted(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> (Vec<Backdrop>, Vec<Quad>) {
    window
        .update(cx, |_, window, _| {
            (window.painted_backdrops(), window.painted_quads())
        })
        .unwrap()
}

#[gpui::test]
fn child_layout_order_parent_clip_and_opacity_reach_the_scene_at_both_scales(
    cx: &mut TestAppContext,
) {
    let window = open(cx, GlassShape::Circle);
    for scale in [1.0, 2.0] {
        cx.simulate_window_scale_factor_change(window.into(), scale);
        cx.run_until_parked();
        let (backdrops, quads) = painted(window, cx);
        assert_eq!(backdrops.len(), 1);
        assert_eq!(quads.len(), 2);
        let glass = backdrops[0];
        let marker = quads
            .iter()
            .find(|quad| quad.bounds.size.width == ScaledPixels(scale))
            .unwrap();
        let background = quads
            .iter()
            .find(|quad| quad.bounds.size.width == ScaledPixels(160.0 * scale))
            .unwrap();
        assert!(background.order < glass.order && glass.order < marker.order);
        assert_eq!(glass.shape, 2);
        assert_eq!(
            glass.bounds,
            Bounds::new(
                point(ScaledPixels(40.0 * scale), ScaledPixels(8.0 * scale)),
                size(ScaledPixels(40.0 * scale), ScaledPixels(40.0 * scale))
            )
        );
        assert_eq!(glass.corner_radius, ScaledPixels(20.0 * scale));
        assert_eq!(glass.blur_sigma, ScaledPixels(3.0 * scale));
        assert_eq!(glass.scale_factor, scale);
        assert_eq!(glass.opacity, 0.4);
        assert_eq!(marker.background.as_solid().unwrap().a, 0.4);
        assert_eq!(glass.content_mask, marker.content_mask);
        assert_eq!(
            glass.content_mask.bounds,
            Bounds::new(
                point(ScaledPixels(10.0 * scale), ScaledPixels(8.0 * scale)),
                size(ScaledPixels(90.0 * scale), ScaledPixels(32.0 * scale))
            )
        );
    }
}

#[gpui::test]
fn accessibility_replaces_glass_without_changing_outline_clip_or_content(cx: &mut TestAppContext) {
    let window = open(cx, GlassShape::Circle);
    for scale in [1.0, 2.0] {
        cx.simulate_window_scale_factor_change(window.into(), scale);
        cx.run_until_parked();
        for shape in [
            GlassShape::Circle,
            GlassShape::Capsule,
            GlassShape::RoundedRectangle {
                corner_radius: 90.0,
            },
        ] {
            window
                .update(cx, |view, _, cx| {
                    view.shape = shape;
                    view.accessibility = GlassAccessibility::default();
                    set_test_preferences(cx, AccessibilityPreferences::default());
                    cx.notify();
                })
                .unwrap();
            cx.run_until_parked();
            let (backdrops, _) = painted(window, cx);
            let glass = backdrops[0];

            for preferences in [
                AccessibilityPreferences {
                    reduce_transparency: true,
                    ..Default::default()
                },
                AccessibilityPreferences {
                    increase_contrast: true,
                    reduce_motion: true,
                    ..Default::default()
                },
            ] {
                cx.update(|cx| set_test_preferences(cx, preferences));
                cx.run_until_parked();
                assert_eq!(cx.read(|cx| cx.reduce_motion()), preferences.reduce_motion);
                let (backdrops, quads) = painted(window, cx);
                assert!(
                    backdrops.is_empty(),
                    "opaque mode still sampled the background"
                );
                assert_eq!(quads.len(), 3);
                let surface = quads
                    .iter()
                    .find(|quad| quad.bounds == glass.bounds)
                    .unwrap();
                assert_eq!(surface.corner_radii, Corners::all(glass.corner_radius));
                assert_eq!(surface.content_mask, glass.content_mask);
                assert_eq!(surface.background.as_solid().unwrap().a, 0.4);
                assert_eq!(
                    surface.border_widths,
                    Edges::all(ScaledPixels(if preferences.increase_contrast {
                        scale
                    } else {
                        0.0
                    }))
                );
                let marker = quads
                    .iter()
                    .find(|quad| quad.bounds.size.width == ScaledPixels(scale))
                    .unwrap();
                assert!(surface.order < marker.order);
                assert_eq!(marker.background.as_solid().unwrap().a, 0.4);
            }

            cx.update(|cx| set_test_preferences(cx, AccessibilityPreferences::default()));
            window
                .update(cx, |view, _, cx| {
                    view.accessibility.reduce_transparency = true;
                    cx.notify();
                })
                .unwrap();
            cx.run_until_parked();
            assert!(
                painted(window, cx).0.is_empty(),
                "explicit host preference was ignored"
            );
            window
                .update(cx, |view, _, cx| {
                    view.accessibility = GlassAccessibility::default();
                    cx.notify();
                })
                .unwrap();
            cx.run_until_parked();
            assert_eq!(
                painted(window, cx).0.len(),
                1,
                "clearing preferences did not restore glass"
            );
            assert!(!cx.read(|cx| cx.reduce_motion()));
        }
    }
}
