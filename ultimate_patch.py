import re
import os

print("M-PLAYLIST [PATCH]: Initiating Ultimate Override...")

# --- 1. GRAPHICS.RS FIX (Color Washout & GPU Letterboxing) ---
graphics_path = r'Z:\M-Playlist\m_playlist\src\graphics.rs'
with open(graphics_path, 'r', encoding='utf-8') as f:
    code = f.read()

# Force Swapchain Downgrade to cure DWM Double Gamma
code = re.sub(
    r'DXGI_FORMAT_R16G16B16A16_FLOAT',
    r'DXGI_FORMAT_B8G8R8A8_UNORM',
    code
)

# Inject Letterbox math and replace PS_Main
start_idx = code.find('float4 PS_Main(')
if start_idx != -1 and 'get_letterbox_uv' not in code:
    end_idx = code.find('}', start_idx) + 1
    
    new_shader = """float2 get_letterbox_uv(float2 uv, uint w, uint h) {
    if (w == 0 || h == 0) return uv;
    float texAspect = (float)w / (float)h;
    float screenAspect = 1920.0 / 1080.0;
    float2 new_uv = uv;
    if (texAspect > screenAspect) {
        float scale = screenAspect / texAspect;
        new_uv.y = (uv.y - 0.5) / scale + 0.5;
    } else if (texAspect < screenAspect) {
        float scale = texAspect / screenAspect;
        new_uv.x = (uv.x - 0.5) / scale + 0.5;
    }
    return new_uv;
}

float4 PS_Main(PS_INPUT input) : SV_TARGET {
    uint wA, hA; texA.GetDimensions(wA, hA);
    float2 uvA = get_letterbox_uv(input.uv, wA, hA);
    float4 colorA = (uvA.x < 0.0 || uvA.x > 1.0 || uvA.y < 0.0 || uvA.y > 1.0) ? float4(0,0,0,0) : texA.Sample(smp, uvA);

    uint wB, hB; texB.GetDimensions(wB, hB);
    float2 uvB = get_letterbox_uv(input.uv, wB, hB);
    float4 colorB = (uvB.x < 0.0 || uvB.x > 1.0 || uvB.y < 0.0 || uvB.y > 1.0) ? float4(0,0,0,0) : texB.Sample(smp, uvB);

    float4 finalColor;
    if (is_overlay_b > 0.5) {
        float4 fg = colorB * blendFactor;
        finalColor.rgb = fg.rgb + (colorA.rgb * (1.0 - fg.a));
        finalColor.a = fg.a + (colorA.a * (1.0 - fg.a));
    } else if (is_overlay_a > 0.5) {
        float alphaA = 1.0 - blendFactor;
        float4 fg = colorA * alphaA;
        finalColor.rgb = fg.rgb + (colorB.rgb * (1.0 - fg.a));
        finalColor.a = fg.a + (colorB.a * (1.0 - fg.a));
    } else {
        finalColor = lerp(colorA, colorB, blendFactor);
    }

    finalColor.rgb *= (brightness + 1.0);
    finalColor.rgb = (finalColor.rgb - 0.5) * (contrast + 1.0) + 0.5;
    float luminance = dot(finalColor.rgb, float3(0.2126, 0.7152, 0.0722));
    finalColor.rgb = lerp(float3(luminance, luminance, luminance), finalColor.rgb, saturation + 1.0);

    float4 subColor = texSub.Sample(smp, input.uv);
    finalColor.rgb = subColor.rgb + (finalColor.rgb * (1.0 - subColor.a));

    return float4(saturate(finalColor.rgb), finalColor.a);
}"""

    code = code[:start_idx] + new_shader + code[end_idx:]

    with open('Z:/M-Playlist/m_playlist/src/graphics.rs', 'w', encoding='utf-8') as f:
        f.write(code)
    print("M-PLAYLIST PATCHED: graphics.rs (Color Space Downgrade & Letterboxing applied)")


# --- 2. MAINWINDOW.XAML.CS FIX (Auto-Advance) ---
cs_path = r'Z:\M-Playlist\MPlaylistApp\MainWindow.xaml.cs'
with open(cs_path, 'r', encoding='utf-8') as f:
    cs_code = f.read()

# Auto-detect listview
lv_match = re.search(r'(\w+)\.SelectedIndex', cs_code)
lv_name = lv_match.group(1) if lv_match else "PlaylistUI"

method_pattern = r'(private\s+void\s+\w+Click\s*\([^)]+\)\s*\{[^\}]*?_conductor\.TransportFireNext\(\);\s*\})'
handler_match = re.search(method_pattern, cs_code)

if handler_match:
    click_header_match = re.search(r'private\s+void\s+(\w+Click)\s*\(', handler_match.group(1))
    click_header = click_header_match.group(0) + 'object sender, RoutedEventArgs e)'
    new_method = click_header + """
        {
            if (_playlist == null || _playlist.Count == 0) return;

            int nextIndex = 0;
            if (""" + lv_name + """.SelectedIndex >= 0) {
                nextIndex = """ + lv_name + """.SelectedIndex + 1;
                if (nextIndex >= _playlist.Count) nextIndex = 0;
            }

            """ + lv_name + """.SelectedIndex = nextIndex;
            """ + lv_name + """.ScrollIntoView(""" + lv_name + """.SelectedItem);

            if (""" + lv_name + """.SelectedItem is MediaCue targetCue) {
                _conductor.SetActiveCue(targetCue);
                _conductor.TransportFireNext();
            }
        }"""
    
    cs_code = cs_code[:handler_match.start()] + new_method + cs_code[handler_match.end():]
    with open(cs_path, 'w', encoding='utf-8') as f:
        f.write(cs_code)
    print("M-PLAYLIST PATCHED: MainWindow.xaml.cs Auto-advance injected.")
