Texture2D src : register(t0);
SamplerState samp : register(s0);
cbuffer Params : register(b0) {
    float4 rect;      // xy: offset, zw: scale of the client area within the capture (uv)
    float2 texel;     // one pixel of the capture, in uv
    float exposure, contrast;
    float saturation, vibrance, temperature, tint;
    float sharpen, vignette, bloom, tonemap;
};

struct VsOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; };

VsOut vs(uint id : SV_VertexID) {
    VsOut o;
    o.uv = float2((id << 1) & 2, id & 2);
    o.pos = float4(o.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}

float3 tap(float2 uv) { return src.SampleLevel(samp, uv, 0).rgb; }

float4 ps(VsOut i) : SV_Target {
    float2 t = rect.xy + i.uv * rect.zw;
    float3 c = tap(t);
    if (sharpen > 0) {
        float3 n = tap(t + float2(texel.x, 0)) + tap(t - float2(texel.x, 0)) + tap(t + float2(0, texel.y)) + tap(t - float2(0, texel.y));
        c = saturate(c + (c - n * 0.25) * sharpen * 1.5);
    }
    if (bloom > 0) {
        float3 b = 0;
        [unroll] for (int k = 0; k < 12; k++) {
            float a = k * 0.5236;
            float2 o = float2(cos(a), sin(a)) * texel * (6 + (k % 3) * 6);
            float3 s = tap(t + o);
            b += max(s - 0.6, 0);
        }
        c += b / 12 * bloom * 2.5;
    }
    c *= exp2(exposure);
    c *= float3(1 + temperature * 0.12 + tint * 0.04, 1 - tint * 0.08, 1 - temperature * 0.12 + tint * 0.04);
    if (tonemap > 0) {
        float3 f = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14);
        c = lerp(c, f, tonemap);
    }
    c = (c - 0.5) * contrast + 0.5;
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    c = lerp(l.xxx, c, saturation);
    float s = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    c = lerp(l.xxx, c, 1 + vibrance * (1 - saturate(s)));
    float2 d = i.uv - 0.5;
    c *= 1 - vignette * saturate(dot(d, d) * 2.2);
    return float4(saturate(c), 1);
}
