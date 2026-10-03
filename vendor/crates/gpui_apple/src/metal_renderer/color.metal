// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

#include <metal_stdlib>
using namespace metal;

struct ColorStage {
    float4 values[3];
    uint operation;
    uint channel;
    uint2 padding;
};

float3 convert_color(float3 color, constant ColorStage *stages, uint count) {
    for (uint index = 0; index < count; ++index) {
        constant ColorStage &stage = stages[index];
        float4 p = stage.values[0];
        float4 q = stage.values[1];
        if (stage.operation == 0) {
            float4 input(color, 1.0f);
            color = float3(dot(input, p), dot(input, q), dot(input, stage.values[2]));
            continue;
        }
        float x = color[stage.channel];
        float gamma = p.x, a = p.y, b = p.z, c = p.w;
        float d = q.x, e = q.y, f = q.z;
        float y;
        if (stage.operation == 1) {
            y = pow(max(x, 0.0f), gamma);
        } else if (stage.operation <= 3) {
            y = x < -b / a ? 0.0f : pow(max(a * x + b, 0.0f), gamma);
            if (stage.operation == 3) y += c;
        } else {
            y = x < d ? c * x : pow(max(a * x + b, 0.0f), gamma);
            if (stage.operation == 5) y += x < d ? f : e;
        }
        color[stage.channel] = y;
    }
    return color;
}

float3 encode_unorm(float3 color) {
    return rint(saturate(color) * 255.0f) / 255.0f;
}

kernel void color_to_display(texture2d<float, access::read> source [[texture(0)]],
                              texture2d<float, access::write> output [[texture(1)]],
                              constant ColorStage *stages [[buffer(0)]],
                              constant uint &count [[buffer(1)]],
                              uint2 pixel [[thread_position_in_grid]]) {
    float4 input = source.read(pixel);
    // WindowServer quantizes both sides of its public color conversion to half precision.
    // A half UNORM accessor can retain float precision; use an explicit conversion here.
    half3 color = half3(convert_color(float3(half3(input.rgb)), stages, count));
    output.write(float4(encode_unorm(float3(color)), float(input.a)), pixel);
}

kernel void color_to_layer(texture2d<float, access::read> source [[texture(0)]],
                            texture2d<float, access::write> output [[texture(1)]],
                            constant ColorStage *stages [[buffer(0)]],
                            constant uint &count [[buffer(1)]],
                            uint2 pixel [[thread_position_in_grid]]) {
    float4 input = source.read(pixel);
    output.write(float4(encode_unorm(convert_color(input.rgb, stages, count)), input.a), pixel);
}
