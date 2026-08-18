import re

# 1. PATCH PLAYLIST.RS (State Toggle)
with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'r', encoding='utf-8') as f:
    code = f.read()
if 'self.is_deck_a_active = is_deck_a;' not in code:
    # Safely inject before the final closing brace of fire_cue.
    pattern = re.compile(r'(println!\("M-Playlist \[WARNING\]: Unhandled Modality.*?\}\s*\})', re.DOTALL)
    code = pattern.sub(r'\1\n\n        // 🚨 CRITICAL: Physically advance the A/B deck state machine\n        self.is_deck_a_active = is_deck_a;', code)
    with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'w', encoding='utf-8') as f:
        f.write(code)
    print("M-PLAYLIST [PATCH]: playlist.rs State Toggle injected.")

# 2. PATCH GRAPHICS.RS (Color Space Linearization & GPU Letterboxing)
with open('Z:/M-Playlist/m_playlist/src/graphics.rs', 'r', encoding='utf-8') as f:
    code = f.read()

# 2a. Inject Letterboxing Function into HLSL
if 'float2 get_letterbox_uv' not in code:
    lb_func = """
float2 get_letterbox_uv(float2 uv, uint w, uint h) {
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
float4 PS_Main"""
    code = code.replace("float4 PS_Main", lb_func, 1)

# 2b. Apply Letterboxing to texA and texB sampling
if 'get_letterbox_uv(uvA' not in code:
    code = re.sub(
        r'float4 colorA = texA\.Sample\(smp,\s*uvA\);',
        r'uint wA, hA; texA.GetDimensions(wA, hA);\n    float2 uvA_lb = get_letterbox_uv(uvA, wA, hA);\n    float4 colorA = (uvA_lb.x < 0.0 || uvA_lb.x > 1.0 || uvA_lb.y < 0.0 || uvA_lb.y > 1.0) ? float4(0,0,0,0) : texA.Sample(smp, uvA_lb);',
        code
    )
    code = re.sub(
        r'float4 colorB = texB\.Sample\(smp,\s*uvB\);',
        r'uint wB, hB; texB.GetDimensions(wB, hB);\n    float2 uvB_lb = get_letterbox_uv(uvB, wB, hB);\n    float4 colorB = (uvB_lb.x < 0.0 || uvB_lb.x > 1.0 || uvB_lb.y < 0.0 || uvB_lb.y > 1.0) ? float4(0,0,0,0) : texB.Sample(smp, uvB_lb);',
        code
    )

# 2c. Apply Color Space Linearization
if 'pow(abs(finalColor.rgb)' not in code:
    pattern = re.compile(r'(return\s+float4\s*\(\s*saturate\s*\(\s*finalColor\.rgb\s*\)\s*,\s*finalColor\.a\s*\)\s*;)')
    code = pattern.sub(r'// OS Physics: Convert to Linear scRGB for FP16 Swapchain\n    finalColor.rgb = pow(abs(finalColor.rgb), 2.2);\n\n    \1', code)

with open('Z:/M-Playlist/m_playlist/src/graphics.rs', 'w', encoding='utf-8') as f:
    f.write(code)
print("M-PLAYLIST [PATCH]: graphics.rs Letterboxing and EOTF Curve injected.")

# 3. PATCH MAINWINDOW.XAML.CS (Auto-Advance)
with open('Z:/M-Playlist/MPlaylistApp/MainWindow.xaml.cs', 'r', encoding='utf-8') as f:
    code = f.read()

# Match the FireNext_Click or PlayFireNext_Click signature and its body
pattern = re.compile(r'(private\s+void\s+(?:PlayFireNext_Click|FireNext_Click)\s*\(\s*object\s+sender\s*,\s*RoutedEventArgs\s+e\s*\)\s*\{)(.*?)(^\s*\})', re.DOTALL | re.MULTILINE)
match = pattern.search(code)
if match and 'PlaylistView.SelectedIndex++' not in match.group(2) and 'nextIndex' not in match.group(2):
    replacement = """
            if (_playlist == null || _playlist.Count == 0) return;
            
            // 1. Strict Auto-Advance Math
            int nextIndex = 0;
            if (PlaylistView.SelectedIndex >= 0) {
                nextIndex = PlaylistView.SelectedIndex + 1;
                if (nextIndex >= _playlist.Count) nextIndex = 0;
            }
            PlaylistView.SelectedIndex = nextIndex;
            PlaylistView.ScrollIntoView(PlaylistView.SelectedItem);
            
            // 2. Extract and Fire
            if (PlaylistView.SelectedItem is MediaCue targetCue) {
                _conductor.LoadCue(targetCue);
                _conductor.TransportFireNext();
            }
"""
    code = code[:match.start(2)] + replacement + code[match.start(3):]
    with open('Z:/M-Playlist/MPlaylistApp/MainWindow.xaml.cs', 'w', encoding='utf-8') as f:
        f.write(code)
    print("M-PLAYLIST [PATCH]: MainWindow.xaml.cs Auto-Advance logic injected.")
else:
    print("M-PLAYLIST [PATCH]: Auto-Advance already present or method not found.")
