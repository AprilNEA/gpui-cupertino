// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

//! Check GPU color conversion against the public ColorSync CPU implementation.
#![cfg(target_os = "macos")]

#[test]
fn display_conversion_matches_public_colorsync_with_half_boundaries() {
    gpui_apple::metal_renderer::MetalRenderer::verify_color_conversion().unwrap();
}
