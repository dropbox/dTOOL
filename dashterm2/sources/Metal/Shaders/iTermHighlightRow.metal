//
//  iTermHighlightRow.metal
//  DashTerm2
//
//  Created by George Nachman on 11/19/17.
//

#include <metal_stdlib>
using namespace metal;

#import "iTermShaderTypes.h"

typedef struct {
    float4 clipSpacePosition [[position]];
    float4 color;
} iTermHighlightRowVertexFunctionOutput;

vertex iTermHighlightRowVertexFunctionOutput
iTermHighlightRowVertexShader(uint vertexID [[ vertex_id ]],
                              constant iTermVertex *vertexArray [[ buffer(iTermVertexInputIndexVertices) ]],
                              constant vector_uint2 *viewportSizePointer  [[ buffer(iTermVertexInputIndexViewportSize) ]],
                              constant iTermHighlightRowPIU *perInstanceUniforms [[ buffer(iTermVertexInputIndexPerInstanceUniforms) ]],
                              unsigned int iid [[instance_id]]) {
    iTermHighlightRowVertexFunctionOutput out;

    float2 pixelSpacePosition = vertexArray[vertexID].position.xy;
    // Apply per-instance Y offset for row positioning
    pixelSpacePosition.y += perInstanceUniforms[iid].yOffset;
    float2 viewportSize = float2(*viewportSizePointer);

    out.clipSpacePosition.xy = pixelSpacePosition / viewportSize;
    out.clipSpacePosition.z = 0.0;
    out.clipSpacePosition.w = 1;

    // Pass color from PIU to fragment shader
    out.color = perInstanceUniforms[iid].color;

    return out;
}

fragment float4
iTermHighlightRowFragmentShader(iTermHighlightRowVertexFunctionOutput in [[stage_in]]) {
    return in.color;
}
