#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

#import "iTermShaderTypes.h"

typedef struct {
    float4 clipSpacePosition [[position]];
    float2 textureCoordinate;
} iTermTimestampsVertexFunctionOutput;

// Instanced vertex shader - uses PIU for per-row y offset
vertex iTermTimestampsVertexFunctionOutput
iTermTimestampsVertexShader(uint vertexID [[ vertex_id ]],
                            uint instanceID [[ instance_id ]],
                            constant iTermVertex *vertexArray [[ buffer(iTermVertexInputIndexVertices) ]],
                            constant vector_uint2 *viewportSizePointer  [[ buffer(iTermVertexInputIndexViewportSize) ]],
                            constant iTermTimestampPIU *pius [[ buffer(iTermVertexInputIndexPerInstanceUniforms) ]]) {
    iTermTimestampsVertexFunctionOutput out;

    float2 pixelSpacePosition = vertexArray[vertexID].position.xy;

    // Apply per-instance offsets from PIU
    pixelSpacePosition.x += pius[instanceID].xOffset;
    pixelSpacePosition.y += pius[instanceID].yOffset;

    float2 viewportSize = float2(*viewportSizePointer);

    out.clipSpacePosition.xy = pixelSpacePosition / viewportSize;
    out.clipSpacePosition.z = 0.0;
    out.clipSpacePosition.w = 1;
    out.textureCoordinate = vertexArray[vertexID].textureCoordinate;

    return out;
}

fragment float4
iTermTimestampsFragmentShader(iTermTimestampsVertexFunctionOutput in [[stage_in]],
                              texture2d<float> texture [[ texture(iTermTextureIndexPrimary) ]]) {
    constexpr sampler textureSampler(mag_filter::linear,
                                     min_filter::linear);

    float4 colorSample = texture.sample(textureSampler, in.textureCoordinate);
    return colorSample;
}
