// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

#include <metal_stdlib>
using namespace metal;

// Distance and optical direction are separate; the direction is not the distance gradient.
float3 continuous_corner_field(float2 point, float2 half_size, float radius) {
    float2 v = abs(point) - half_size;
    float expanded = 1.528665 * radius;
    float2 direction_point = v;
    float distance;
    if (expanded == 0) {
        distance = length(max(v, 0.0)) + min(max(v.x, v.y), 0.0);
    } else {
        float2 blend = saturate((expanded - half_size) / (expanded - radius));
        direction_point += mix(expanded, radius, max(blend.x, blend.y));
        // Keep the radial calculation in pixels so tiny radii cannot overflow normalized coordinates.
        float2 q = max(v + expanded, 0.0);
        float largest = max(q.x, q.y);
        float ratio = largest > 0 ? min(q.x, q.y) / largest : 0;
        float radial = length(q);
        float polynomial = (((3.1560099124908447 - 0.9260540008544922 * ratio)
            * ratio - 3.6412200927734375) * ratio + 1.268030047416687) * ratio + 0.2685309946537018;
        float continuous = radial + expanded - expanded /
            (1 - ratio * ratio * (min(radial, expanded) / expanded) * polynomial);
        float circular = 0.6541655659675598 * length(max(1.528665 * q - 0.5286650061607361 * expanded, 0.0))
            + 0.3458344340324402 * expanded;
        float2 axes = mix(float2(continuous), float2(circular), blend);
        float angular = q.y > q.x ? saturate(ratio - 0.5) : saturate(1.5 - ratio);
        float radial_distance = mix(axes.x, axes.y, angular) - expanded;
        // Preserve native half quantization where representable; retain float outside half's range.
        if (abs(radial_distance) <= 65504.0 * expanded) {
            radial_distance = float(half(radial_distance / expanded)) * expanded;
        }
        distance = min(max(direction_point.x, direction_point.y), 0.0) + radial_distance;
    }
    float2 positive = max(direction_point, 0.0);
    float magnitude = length(positive);
    float2 direction = magnitude > 0 ? positive / magnitude :
        (direction_point.x > direction_point.y ? float2(1, 0) : float2(0, 1));
    return float3(distance, direction * select(float2(-1), float2(1), point >= 0));
}

float continuous_corner_coverage(float distance) {
    return saturate(0.5 - distance / max(fwidth(distance), 0.0001));
}
