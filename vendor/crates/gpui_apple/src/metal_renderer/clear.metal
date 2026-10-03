// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

#include <metal_stdlib>
using namespace metal;

struct ClearUniforms {
    float4 shape;
    float4 displacement;
    half4 face[3];
    float blur_radius;
    float opacity;
    float4 clip;
};

struct ClearInput {
    float4 position [[attribute(0)]];
    float2 sdf_uv [[attribute(1)]];
    float2 src_uv [[attribute(2)]];
};

struct ClearVertex {
    float4 position [[position]];
    float2 sdf_uv;
    float2 src_uv;
};

vertex ClearVertex clear_vertex(ClearInput in [[stage_in]],
                                 constant float4x4 &mvp [[buffer(2)]]) {
    return {mvp * in.position, in.sdf_uv, in.src_uv};
}

fragment half4 clear_capture(float4 position [[position]],
                              texture2d<half, access::sample> source [[texture(0)]],
                              constant float4 *u [[buffer(0)]]) {
    float2 pixel = min(u[0].xy + position.xy * 2, u[0].zw);
    // Keep the blur in bottom-origin coordinates, including its compression blocks.
    pixel.y = u[1].z - pixel.y;
    constexpr sampler linear_clamp(coord::normalized, address::clamp_to_edge,
                                    filter::linear, mip_filter::none);
    return source.sample(linear_clamp, pixel * u[1].xy);
}

fragment half4 clear_copy_base(float4 position [[position]],
                                texture2d<half, access::sample> source [[texture(0)]],
                                constant CopyUniforms &u [[buffer(0)]]) {
    return read_clamped(source, short2(position.xy) + u.base, u);
}

inline half3 clear_field(ClearVertex in, constant ClearUniforms &u) {
    half3 field;
    if (u.shape.z == 4) {
        field = half3(continuous_corner_field(in.sdf_uv, u.shape.xy, u.shape.w));
    } else {
        half2 offset = half2(abs(in.sdf_uv) - u.shape.xy);
        half2 axis = offset.x > offset.y ? half2(1, 0) : half2(0, 1);
        half2 sign = select(half2(-1), half2(1), in.sdf_uv >= 0);
        field = half3(max(offset.x, offset.y), axis * sign);
    }
    return field;
}

inline half clear_coverage(ClearVertex in, half3 field, constant ClearUniforms &u) {
    if (u.opacity == 0 || any(in.position.xy < u.clip.xy) || any(in.position.xy >= u.clip.zw)) discard_fragment();
    constexpr half epsilon = as_type<half>(ushort(0x068e));
    half coverage = half(saturate(float(-field.x / max(fwidth(field.x), epsilon)) + 0.5f));
    if (coverage == 0) discard_fragment();
    return coverage;
}

half4 clear_color(ClearVertex in, texture2d<half, access::sample> source,
                  constant ClearUniforms &u) {
    half3 field = clear_field(in, u);
    half coverage = clear_coverage(in, field, u);
    half2 direction = field.yz * rsqrt(dot(field.yz, field.yz));
    constexpr half epsilon = as_type<half>(ushort(0x068e));
    half2 gradient = half2(dot(float2(direction), u.displacement.xy),
                           dot(float2(direction), u.displacement.zw));
    half t = saturate(half(1.0f / 20) * -field.x);
    half profile = saturate(sqrt((2.0h - t) * t));
    half displacement = -60.0h - profile * -60.0h;
    // Convert UV to half before displacement. Moving this boundary changes sampled colors.
    half2 uv = half2(in.src_uv) + displacement * gradient;
    half radius = half(u.blur_radius);
    half lod_radius = radius < 2.0h ? half(1.0f + float(radius) * 0.5f) : radius;
    half lod = max(0.0h, log2(lod_radius));
    constexpr sampler trilinear_clamp(coord::normalized, address::clamp_to_edge,
                                      filter::linear, mip_filter::linear);
    half4 sampled = source.sample(trilinear_clamp, float2(uv), level(float(lod)));
    sampled.rgb = select(sampled.rgb, half3(0), abs(sampled.rgb) < epsilon);
    half3 color = sampled.rgb / max(sampled.a, epsilon);
    half3 transformed = half3(dot(color, u.face[0].xyz), dot(color, u.face[1].xyz),
                              dot(color, u.face[2].xyz));
    transformed += half3(u.face[0].w, u.face[1].w, u.face[2].w);
    half4 result = half4(transformed, 1.0h) * coverage;
    result.rgb = clamp(result.rgb / max(result.a, epsilon), half3(-0.75h), half3(1.0h)) * result.a;
    return result * half(u.opacity);
}

fragment half4 clear_fragment(ClearVertex in [[stage_in]],
                               texture2d<half, access::sample> source [[texture(3)]],
                               constant ClearUniforms &u [[buffer(1)]]) {
    return clear_color(in, source, u);
}

fragment half4 clear_unquantized(ClearVertex in [[stage_in]],
                                 texture2d<half, access::sample> source [[texture(3)]],
                                 texture2d<float, access::read> background [[texture(4)]],
                                 constant ClearUniforms &u [[buffer(1)]]) {
    float4 color = float4(clear_color(in, source, u));
    // Preserve the composite until the layer conversion performs the final 8-bit quantization.
    return half4(saturate(color + background.read(uint2(in.position.xy)) * (1.0f - color.a)));
}

fragment half4 clear_restore(ClearVertex in [[stage_in]],
                              texture2d<half, access::read> source [[texture(3)]],
                              constant ClearUniforms &u [[buffer(1)]]) {
    clear_coverage(in, clear_field(in, u), u);
    // Coverage was already composed in display RGB. Write the converted result exactly once.
    return source.read(uint2(in.position.xy));
}
