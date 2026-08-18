import re

with open('Z:/M-Playlist/m_playlist/src/graphics.rs', 'r', encoding='utf-8') as f:
    code = f.read()

if 'pow(abs(finalColor.rgb)' not in code:
    pattern = re.compile(r'(return\s+float4\s*\(\s*saturate\s*\(\s*finalColor\.rgb\s*\)\s*,\s*finalColor\.a\s*\)\s*;)')
    new_code = pattern.sub(r'// OS Physics: Convert to Linear scRGB for FP16 Swapchain\n    finalColor.rgb = pow(abs(finalColor.rgb), 2.2);\n\n    \1', code)
    
    with open('Z:/M-Playlist/m_playlist/src/graphics.rs', 'w', encoding='utf-8') as f:
        f.write(new_code)
    print("M-PLAYLIST [PATCH]: EOTF curve injected successfully.")
else:
    print("M-PLAYLIST [PATCH]: EOTF curve already exists.")
