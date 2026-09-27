//! Regression checks for backdrop ordering, active layers, and paint replay.
#![cfg(target_os = "macos")]

use gpui::{Backdrop, Bounds, ContentMask, PrimitiveBatch, Quad, ScaledPixels, Scene, point, size};

fn bounds(x: f32, width: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(0.)),
        size(ScaledPixels(width), ScaledPixels(20.)),
    )
}

fn quad(x: f32) -> Quad {
    Quad {
        bounds: bounds(x, 10.),
        content_mask: ContentMask {
            bounds: bounds(0., 1000.),
        },
        ..Quad::default()
    }
}

fn backdrop(x: f32) -> Backdrop {
    Backdrop {
        order: 0,
        shape: 0,
        bounds: bounds(x, 10.),
        content_mask: ContentMask {
            bounds: bounds(0., 1000.),
        },
        corner_radius: ScaledPixels(0.),
        blur_sigma: ScaledPixels(0.),
        tint: [0.; 4],
        saturation: 1.,
        brightness: 0.,
        refraction_amount: ScaledPixels(0.),
        refraction_width: ScaledPixels(1.),
        direction_mix: 0.,
        dispersion: ScaledPixels(0.),
        highlight: 0.,
        edge_bleed: 0.,
        scale_factor: 1.,
        opacity: 1.,
    }
}

/// Each entry lists the quads visible when a backdrop captures the framebuffer.
fn captured_quads(scene: &Scene) -> Vec<Vec<u32>> {
    let mut drawn = Vec::new();
    let mut captures = Vec::new();
    for batch in scene.batches() {
        match batch {
            PrimitiveBatch::Quads(range) => drawn.extend(
                scene.quads[range]
                    .iter()
                    .map(|quad| quad.bounds.origin.x.0 as u32),
            ),
            PrimitiveBatch::Backdrop(_) => {
                let mut snapshot = drawn.clone();
                snapshot.sort_unstable();
                captures.push(snapshot);
            }
            _ => panic!("unexpected primitive in test scene"),
        }
    }
    captures
}

#[test]
fn backdrop_captures_all_preceding_disjoint_primitives() {
    let mut scene = Scene::default();
    scene.insert_primitive(quad(0.));
    scene.insert_primitive(quad(30.));
    scene.insert_primitive(backdrop(100.));
    scene.insert_primitive(quad(60.));
    scene.insert_primitive(backdrop(200.));
    scene.insert_primitive(quad(90.));
    scene.finish();
    assert_eq!(captured_quads(&scene), [vec![0, 30], vec![0, 30, 60]]);

    scene.clear();
    scene.insert_primitive(backdrop(100.));
    scene.finish();
    assert_eq!(captured_quads(&scene), [Vec::<u32>::new()]);
    assert_eq!(scene.backdrops[0].order, 1);
}

#[test]
fn backdrop_rebases_nested_layers_and_replayed_operations() {
    let mut scene = Scene::default();
    scene.push_layer(bounds(0., 200.));
    scene.insert_primitive(quad(0.));
    let cached_start = scene.len();
    scene.push_layer(bounds(20., 40.));
    scene.insert_primitive(quad(20.));
    scene.insert_primitive(backdrop(100.));
    scene.insert_primitive(quad(40.));
    scene.pop_layer();
    let cached_end = scene.len();
    scene.insert_primitive(quad(100.));
    scene.pop_layer();
    scene.insert_primitive(quad(300.));
    scene.insert_primitive(backdrop(200.));
    scene.insert_primitive(quad(400.));
    scene.finish();
    assert_eq!(
        captured_quads(&scene),
        [vec![0, 20], vec![0, 20, 40, 100, 300]]
    );

    // Replay inside a different open layer and an already-advanced epoch.
    let mut replayed = Scene::default();
    replayed.insert_primitive(quad(900.));
    replayed.insert_primitive(backdrop(800.));
    replayed.push_layer(bounds(0., 500.));
    replayed.replay(cached_start..cached_end, &scene);
    replayed.insert_primitive(quad(100.));
    replayed.pop_layer();
    replayed.insert_primitive(backdrop(200.));
    replayed.finish();
    assert_eq!(
        captured_quads(&replayed),
        [vec![900], vec![20, 900], vec![20, 40, 100, 900]]
    );
}

#[test]
fn fully_clipped_backdrop_does_not_create_a_barrier() {
    let mut scene = Scene::default();
    let mut clipped = backdrop(100.);
    clipped.content_mask.bounds = bounds(0., 10.);
    scene.insert_primitive(quad(0.));
    scene.insert_primitive(clipped);
    scene.insert_primitive(quad(200.));
    scene.finish();
    assert!(scene.backdrops.is_empty());
    assert_eq!(scene.len(), 2);
    assert!(
        matches!(scene.batches().next(), Some(PrimitiveBatch::Quads(range)) if range == (0..2))
    );
}
