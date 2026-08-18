import re

with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'r', encoding='utf-8') as f:
    code = f.read()

if 'self.is_deck_a_active = is_deck_a;' not in code:
    # Use regex to find the end of the match block inside fire_cue and append the state toggle
    pattern = r'(println!\("M-Playlist \[WARNING\]: Unhandled Modality \{\}", cue\.modality\);\s*\}\s*\})'
    injection = r'\1\n\n        // 🚨 CRITICAL: Physically advance the A/B deck state machine\n        self.is_deck_a_active = is_deck_a;'
    
    new_code = re.sub(pattern, injection, code)
    with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'w', encoding='utf-8') as f:
        f.write(new_code)
    print("M-PLAYLIST [PATCH]: State toggle injected successfully.")
else:
    print("M-PLAYLIST [PATCH]: State toggle already exists.")
