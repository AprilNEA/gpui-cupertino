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
// Truncated sigma=1 Gaussian, normalized including both symmetric sides.
constant float weights[4] = {0.39905028, 0.24203623, 0.05400558, 0.00443305};
kernel void backdrop_horizontal(texture2d<half> source [[texture(0)]], texture2d<half, access::write> target [[texture(1)]], constant uint &stride [[buffer(0)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= target.get_width() || p.y >= target.get_height()) return;
    float2 extent(source.get_width(), source.get_height());
    float2 uv = (float2(p) + 0.5) / extent;
    float4 c = float4(source.sample(filtered, uv)) * weights[0];
    for (int i = 1; i <= 3; ++i) {
        float2 delta(float(i * stride) / extent.x, 0);
        c += (float4(source.sample(filtered, uv + delta)) + float4(source.sample(filtered, uv - delta))) * weights[i];
    }
    target.write(half4(c), p);
}
kernel void backdrop_vertical(texture2d<half> source [[texture(0)]], texture2d<half, access::write> target [[texture(1)]], constant uint &stride [[buffer(0)]], uint2 p [[thread_position_in_grid]]) {
    if (p.x >= target.get_width() || p.y >= target.get_height()) return;
    float2 uv = (float2(p) + 0.5) / float2(target.get_width(), target.get_height());
    float4 c = float4(source.sample(filtered, uv)) * weights[0];
    for (int i = 1; i <= 3; ++i) {
        float2 delta(0, float(i * stride) / source.get_height());
        c += (float4(source.sample(filtered, uv + delta)) + float4(source.sample(filtered, uv - delta))) * weights[i];
    }
    target.write(half4(c), p);
}
vertex BackdropVertex backdrop_vertex(uint id [[vertex_id]], constant BackdropUniforms &u [[buffer(0)]]) {
    constexpr float2 corners[6] = {{0,0},{1,0},{0,1},{0,1},{1,0},{1,1}};
    float2 p = u.bounds.xy + corners[id] * u.bounds.zw;
    return {float4(p.x / u.viewport.x * 2 - 1, 1 - p.y / u.viewport.y * 2, 0, 1)};
}
float2 safe_direction(float2 v) {
    float len = length(v);
    return len > 0.00001 ? v / len : float2(0);
}
float3 shape_field(float2 p, float2 half_size, float radius, int kind) {
    float2 n;
    float d;
    if (kind == 2) {
        d = length(p) - min(half_size.x, half_size.y);
        n = safe_direction(p);
    } else {
        radius = kind == 1 ? min(half_size.x, half_size.y) : clamp(radius, 0.0, min(half_size.x, half_size.y));
        float2 q = abs(p) - half_size + radius;
        d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
        n = any(q > 0.0) ? sign(p) * safe_direction(max(q, 0.0)) : (q.x > q.y ? float2(sign(p.x),0) : float2(0,sign(p.y)));
    }
    return float3(d, n);
}
float profile(float d, float width) {
    float t = saturate(-d / width);
    return 1 - sqrt(t * (2 - t));
}
float4 sample_blur(texture2d_array<half> source, float2 uv, float sigma, float max_level) {
    constexpr float kernel_variance = 0.995912;
    // The CPU selects the adjacent scales; recomputing LOD here could choose
    // an overwritten slice when rounding near an integer scale boundary.
    float high = max_level, low = max(0.0, high - 1);
    float low_variance = kernel_variance * (pow(4.0, low) - 1) / 3;
    float high_variance = kernel_variance * (pow(4.0, high) - 1) / 3;
    float blend = high == low ? 0 : saturate((sigma*sigma - low_variance) / (high_variance - low_variance));
    return mix(float4(source.sample(filtered, uv, uint(low) % 2)), float4(source.sample(filtered, uv, uint(high) % 2)), blend);
}
fragment float4 backdrop_fragment(BackdropVertex v [[stage_in]], constant BackdropUniforms &u [[buffer(0)]], texture2d<float> original [[texture(0)]], texture2d_array<half> pyramid [[texture(1)]]) {
    float2 p = v.position.xy;
    if (any(p < u.clip.xy) || any(p >= u.clip.xy + u.clip.zw)) discard_fragment();
    float2 uv = p / u.viewport.xy;
    float4 background = original.sample(filtered, uv);
    float2 local = p - u.bounds.xy - u.bounds.zw * 0.5;
    float3 field = shape_field(local, u.bounds.zw * 0.5, u.shape.x, int(u.shape.y));
    float coverage = saturate(0.5 - field.x / max(fwidth(field.x), 0.0001)) * u.shape.w;
    if (coverage <= 0) return background;
    if (u.shape.z == 0 && u.tint.a == 0 && u.optics.x == 0 && u.optics.w == 0 && u.color.x == 1 && u.color.y == 0 && u.color.z == 0 && u.color.w == 0) return background;
    float2 radial = safe_direction(local / max(u.bounds.zw * 0.5, float2(0.0001)));
    float2 direction = safe_direction(mix(field.yz, radial, u.optics.z));
    float edge = profile(field.x, u.optics.y);
    float2 refracted = uv + direction * edge * u.optics.x / u.viewport.xy;
    float sigma = u.shape.z;
    float3 color;
    if (u.optics.w > 0) {
        constexpr float3 spectral[7] = {{0.5,0,0},{1.0/3,1.0/9,0},{1.0/6,2.0/9,0},{0,1.0/3,0},{0,2.0/9,1.0/6},{0,1.0/9,1.0/3},{0,0,0.5}};
        color = 0;
        for (int i=0; i<7; ++i) {
            float offset = 1.0 - float(i) / 3;
            color += sample_blur(pyramid, refracted + offset * direction * edge * u.optics.w / u.viewport.xy, sigma, u.viewport.z).rgb * spectral[i];
        }
    } else {
        color = sample_blur(pyramid, refracted, sigma, u.viewport.z).rgb;
    }
    float luminance = dot(color, float3(0.2126,0.7152,0.0722));
    color = mix(float3(luminance), color, u.color.x) + u.color.y;
    color = mix(color, u.tint.rgb, u.tint.a);
    if (u.color.w > 0) {
        // ponytail: bleed shares the optical band; split its width when calibration requires it.
        float2 outside = uv + field.yz * (max(-field.x, 0.0) + u.viewport.w) / u.viewport.xy;
        float3 bleed = sample_blur(pyramid, outside, sigma, u.viewport.z).rgb;
        color = mix(color, bleed, u.color.w * edge * edge);
    }
    float highlight = saturate(1 + field.x / (1.5 * u.viewport.w)) * saturate(dot(field.yz, normalize(float2(-0.6,-0.8)))) * u.color.z;
    color = mix(color, float3(1), highlight);
    // Known opaque SDR background; all optical/filter work occurs in linear RGB.
    return float4(to_srgb(mix(to_linear(background.rgb), color, coverage)), background.a);
}
