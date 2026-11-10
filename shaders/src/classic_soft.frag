#version 450

layout(set = 2, binding = 0) uniform sampler2D screenTexture;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 outColor;

const vec2 TEXEL = vec2(1.0 / 320.0, 1.0 / 240.0);

const float BLEND_STRENGTH = 0.30;
const float BLOOM_STRENGTH = 0.18;
const float BLOOM_THRESHOLD = 0.55;

vec3 sampleScreen(vec2 offset) {
    return texture(screenTexture, uv + offset * TEXEL).rgb;
}

float luminance(vec3 color) {
    return dot(color, vec3(0.2126, 0.7152, 0.0722));
}

void main() {
    vec3 original = sampleScreen(vec2(0.0));

    // Смешивание соседних пикселей
    vec3 neighbors =
        sampleScreen(vec2(-1.0, 0.0)) +
        sampleScreen(vec2( 1.0, 0.0)) +
        sampleScreen(vec2(0.0, -1.0)) +
        sampleScreen(vec2(0.0,  1.0));

    neighbors *= 0.25;

    vec3 blended = mix(
        original,
        neighbors,
        BLEND_STRENGTH
    );

    // Небольшой локальный Bloom
    vec3 glow = vec3(0.0);
    float weightSum = 0.0;

    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            vec2 offset = vec2(float(x), float(y));
            vec3 color = sampleScreen(offset);

            float bright = smoothstep(
                BLOOM_THRESHOLD,
                1.0,
                luminance(color)
            );

            float weight = exp(-dot(offset, offset) / 3.0);

            glow += color * bright * weight;
            weightSum += weight;
        }
    }

    glow /= weightSum;

    vec3 result = blended + glow * BLOOM_STRENGTH;

    outColor = vec4(clamp(result, 0.0, 1.0), 1.0);
}
