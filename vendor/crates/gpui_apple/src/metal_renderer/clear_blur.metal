// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

#include <metal_stdlib>
using namespace metal;

struct CopyUniforms {
    short2 base;
    short4 clamp;
    ushort2 base_size;
    ushort2 destination_size;
    ushort destination_level;
    bool no_base_mip;
};

struct DownsampleUniforms {
    ushort source_level;
    ushort destination_level;
    ushort width;
    ushort height;
    float dx;
    float dy;
};

struct BlurPixel {
    half4 data;
};

vertex float4 fullscreen(uint index [[vertex_id]]) {
    const float2 positions[] = {float2(-1, -1), float2(3, -1), float2(-1, 3)};
    return float4(positions[index], 0, 1);
}

fragment half4 store_blur_half(float4 position [[position]],
                               texture2d<half, access::read> source [[texture(0)]],
                               constant uint &source_level [[buffer(0)]]) {
    return source.read(uint2(position.xy), source_level);
}

inline half4 clean_rgb(half4 value) {
    value.rgb = select(value.rgb, half3(0), abs(value.rgb) < half3(as_type<half>(ushort(0x068e))));
    return value;
}

inline half4 read_clamped(texture2d<half, access::sample> source, short2 coordinate,
                          constant CopyUniforms &u) {
    return source.read(ushort2(clamp(coordinate, u.clamp.xy, u.clamp.zw)), ushort(0));
}

inline half4 averaged_cell(texture2d<half, access::sample> source, short2 cell,
                           constant CopyUniforms &u) {
    short2 base = 2 * cell + u.base;
    return (((read_clamped(source, base + short2(1, 0), u)
            + read_clamped(source, base, u))
            + read_clamped(source, base + short2(0, 1), u))
            + read_clamped(source, base + short2(1, 1), u)) * 0.25h;
}

inline half4 filter_cache(threadgroup const half4 *cache, ushort center, ushort pitch) {
    half4 top_outer = cache[center - 2 * pitch];
    half4 top_left = cache[center - pitch - 1];
    half4 top_right = cache[center - pitch + 1];
    half4 top = cache[center - pitch];
    half4 left_outer = cache[center - 2];
    half4 right_outer = cache[center + 2];
    half4 left = cache[center - 1];
    half4 right = cache[center + 1];
    half4 middle = cache[center];
    half4 bottom_left = cache[center + pitch - 1];
    half4 bottom_right = cache[center + pitch + 1];
    half4 diagonal = ((top_right + top_left) + bottom_left) + bottom_right;
    half4 bottom = cache[center + pitch];
    half4 inner = ((left + top) + right) + bottom;
    half4 bottom_outer = cache[center + 2 * pitch];
    half4 outer = ((left_outer + top_outer) + right_outer) + bottom_outer;
    half4 weighted_outer = outer * 0.05633544921875h;
    half4 weighted_diagonal = diagonal * 0.07708740234375h;
    half4 weighted_inner = inner * 0.0902099609375h;
    half4 weighted_center = middle * 0.10546875h;
    return clean_rgb(((weighted_diagonal + weighted_center) + weighted_inner) + weighted_outer);
}

// Cache storage preserves the half-precision boundary before the weighted sum.
kernel void copy_base_compute(
    imageblock<BlurPixel, imageblock_layout_explicit> block,
    ushort2 tid [[thread_position_in_threadgroup]],
    ushort2 group [[threadgroup_position_in_grid]],
    texture2d<half, access::sample> source [[texture(0)]],
    texture2d<half, access::write> destination [[texture(1)]],
    constant CopyUniforms &u [[buffer(0)]]) {
    threadgroup half4 cache[400];
    ushort2 origin = 16 * group;
    cache[tid.y * 20 + tid.x] = averaged_cell(source, short2(origin + tid) - 2, u);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (all(tid < ushort2(16))) {
        block.data(tid)->data = filter_cache(cache, (tid.y + 2) * 20 + tid.x + 2, 20);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup_imageblock);
    if (all(tid == ushort2(0))) {
        destination.write(block.slice(block.data(ushort2(0))->data, ushort2(16)), origin,
                          u.destination_level);
    }
}

kernel void downsample_compute(
    imageblock<BlurPixel, imageblock_layout_explicit> block,
    ushort2 gid [[thread_position_in_grid]],
    ushort2 tid [[thread_position_in_threadgroup]],
    ushort2 group_size [[threads_per_threadgroup]],
    texture2d<half, access::sample> source [[texture(0)]],
    texture2d<half, access::write> destination [[texture(1)]],
    constant DownsampleUniforms &u [[buffer(0)]]) {
    threadgroup half4 cache[720];
    ushort2 origin = gid - tid;
    ushort2 active_size = min(group_size, ushort2(u.width, u.height) - origin);
    ushort2 cache_size = active_size + 4;
    ushort2 cache_cell = 2 * tid;
    if (all(cache_cell < cache_size)) {
        constexpr sampler linear_clamp(coord::normalized, address::clamp_to_edge,
                                       filter::linear, mip_filter::nearest);
        float2 uv = (float2(origin + cache_cell) - 1.5f) * float2(u.dx, u.dy);
        ushort index = cache_cell.y * cache_size.x + cache_cell.x;
        cache[index] = source.sample(linear_clamp, uv, level(float(u.source_level)));
        cache[index + 1] = source.sample(linear_clamp, uv + float2(u.dx, 0),
                                         level(float(u.source_level)));
        cache[index + cache_size.x] = source.sample(linear_clamp, uv + float2(0, u.dy),
                                                    level(float(u.source_level)));
        cache[index + cache_size.x + 1] = source.sample(linear_clamp,
            (uv + float2(u.dx, 0)) + float2(0, u.dy), level(float(u.source_level)));
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (all(gid < ushort2(u.width, u.height))) {
        block.data(tid)->data = filter_cache(cache,
            (tid.y + 2) * cache_size.x + tid.x + 2, cache_size.x);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup_imageblock);
    if (all(tid == ushort2(0))) {
        destination.write(block.slice(block.data(ushort2(0))->data, group_size), gid,
                          u.destination_level);
    }
}
