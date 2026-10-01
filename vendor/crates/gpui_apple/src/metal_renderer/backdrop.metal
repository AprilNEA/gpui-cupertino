#include <metal_stdlib>
using namespace metal;

struct BackdropUniforms {
    float4 bounds, clip, tint, shape, optics, color, viewport;
};
struct BackdropVertex { float4 position [[position]]; };
constexpr sampler filtered(coord::normalized, address::clamp_to_edge, filter::linear, mip_filter::linear);

float3 to_linear(float3 c) {
    return select(c / 12.92, pow((c + 0.055) / 1.055, float3(2.4)), c > 0.04045);
}
float3 to_srgb(float3 c) {
    c = max(c, 0.0);
    return select(12.92 * c, 1.055 * pow(c, float3(1.0 / 2.4)) - 0.055, c > 0.0031308);
}
kernel void backdrop_copy(texture2d<float, access::read> source [[texture(0)]], texture2d<half, access::write> target [[texture(1)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= target.get_width() || p.y >= target.get_height()) return;
    float4 c = source.read(p);
    target.write(half4(float4(to_linear(c.rgb), c.a)), p);
}
kernel void backdrop_horizontal(texture2d<half> source [[texture(0)]], texture2d<half, access::write> target [[texture(1)]], constant uint &count [[buffer(0)]], device const float2 *taps [[buffer(1)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= target.get_width() || p.y >= target.get_height()) return;
    float2 extent(source.get_width(), source.get_height());
    float2 uv = (float2(p) + 0.5) / extent;
    float4 c = float4(source.sample(filtered, uv)) * taps[0].y;
    for (uint i = 1; i < count; ++i) {
        float2 delta(taps[i].x / extent.x, 0);
        c += (float4(source.sample(filtered, uv + delta)) + float4(source.sample(filtered, uv - delta))) * taps[i].y;
    }
    target.write(half4(c), p);
}
kernel void backdrop_vertical(texture2d<half> source [[texture(0)]], texture2d<half, access::write> target [[texture(1)]], constant uint &count [[buffer(0)]], device const float2 *taps [[buffer(1)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= target.get_width() || p.y >= target.get_height()) return;
    float2 uv = (float2(p) + 0.5) / float2(target.get_width(), target.get_height());
    float4 c = float4(source.sample(filtered, uv)) * taps[0].y;
    for (uint i = 1; i < count; ++i) {
        float2 delta(0, taps[i].x / source.get_height());
        c += (float4(source.sample(filtered, uv + delta)) + float4(source.sample(filtered, uv - delta))) * taps[i].y;
    }
    target.write(half4(c), p);
}
vertex BackdropVertex backdrop_vertex(uint id [[vertex_id]], constant BackdropUniforms &u [[buffer(0)]]) {
    constexpr float2 corners[6] = {{0,0},{1,0},{0,1},{0,1},{1,0},{1,1}};
    // The SDF's antialias fringe extends outside the geometric bounds.
    float2 p = u.bounds.xy - 1 + corners[id] * (u.bounds.zw + 2);
    return {float4(p.x / u.viewport.x * 2 - 1, 1 - p.y / u.viewport.y * 2, 0, 1)};
}
float2 safe_direction(float2 v) {
    float len = length(v);
    return len > 0.00001 ? v / len : float2(0);
}
float3 shape_field(float2 p, float2 half_size, float radius, int kind) {
    if (kind == 2) half_size = float2(min(half_size.x, half_size.y));
    radius = kind == 0 ? clamp(radius, 0.0, min(half_size.x, half_size.y)) : min(half_size.x, half_size.y);
    return continuous_corner_field(p, half_size, radius);
}
float profile(float d, float width) {
    float t = saturate(-d / width);
    return 1 - sqrt(t * (2 - t));
}
fragment float4 backdrop_fragment(BackdropVertex v [[stage_in]], constant BackdropUniforms &u [[buffer(0)]], texture2d<float> original [[texture(0)]], texture2d<half> material_source [[texture(1)]]) {
    float2 p = v.position.xy;
    if (any(p < u.clip.xy) || any(p >= u.clip.xy + u.clip.zw)) discard_fragment();
    float2 uv = p / u.viewport.xy;
    float4 background = original.sample(filtered, uv);
    float2 local = p - u.bounds.xy - u.bounds.zw * 0.5;
    float3 field = shape_field(local, u.bounds.zw * 0.5, u.shape.x, int(u.shape.y));
    float coverage = continuous_corner_coverage(field.x);
    if (coverage <= 0 || u.shape.w == 0) return background;
    if (u.shape.z == 0 && u.tint.a == 0 && u.optics.x == 0 && u.optics.w == 0 && u.color.x == 1 && u.color.y == 0 && u.color.z == 0 && u.color.w == 0) return background;
    float2 radial = safe_direction(local / max(u.bounds.zw * 0.5, float2(0.0001)));
    float2 direction = safe_direction(mix(field.yz, radial, u.optics.z));
    float edge = profile(field.x, u.optics.y);
    float2 refracted = uv + direction * edge * u.optics.x / u.viewport.xy;
    float3 color;
    if (u.optics.w > 0) {
        constexpr float3 spectral[7] = {{0.5,0,0},{1.0/3,1.0/9,0},{1.0/6,2.0/9,0},{0,1.0/3,0},{0,2.0/9,1.0/6},{0,1.0/9,1.0/3},{0,0,0.5}};
        color = 0;
        for (int i=0; i<7; ++i) {
            float offset = 1.0 - float(i) / 3;
            color += float3(material_source.sample(filtered, refracted + offset * direction * edge * u.optics.w / u.viewport.xy).rgb) * spectral[i];
        }
    } else {
        color = float3(material_source.sample(filtered, refracted).rgb);
    }
    float luminance = dot(color, float3(0.2126,0.7152,0.0722));
    color = mix(float3(luminance), color, u.color.x) + u.color.y;
    color = mix(color, u.tint.rgb, u.tint.a);
    if (u.color.w > 0) {
        // ponytail: bleed shares the optical band; split its width when calibration requires it.
        float2 outside = uv + field.yz * (max(-field.x, 0.0) + u.viewport.w) / u.viewport.xy;
        float3 bleed = float3(material_source.sample(filtered, outside).rgb);
        color = mix(color, bleed, u.color.w * edge * edge);
    }
    float highlight = saturate(1 + field.x / (1.5 * u.viewport.w)) * saturate(dot(field.yz, normalize(float2(-0.6,-0.8)))) * u.color.z;
    color = mix(color, float3(1), highlight);
    // Ancestor opacity remains linear; geometric edge coverage blends encoded window values.
    float3 face = to_srgb(mix(to_linear(background.rgb), color, u.shape.w));
    return float4(mix(background.rgb, face, coverage), background.a);
}
