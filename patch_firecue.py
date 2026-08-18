import re

with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'r', encoding='utf-8') as f:
    code = f.read()

# Locate the premature state toggle at the end of fire_cue
target = r'\s*// 🚨 CRITICAL: Physically advance the A/B deck state machine\s*self\.is_deck_a_active = is_deck_a;'
fallback = r'\s*//.*CRITICAL.*Physically advance the A/B deck state machine\s*self\.is_deck_a_active = is_deck_a;'
fallback_2 = r'\s*self\.is_deck_a_active = is_deck_a;'

if re.search(target, code):
    new_code = re.sub(target, '', code)
    with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'w', encoding='utf-8') as f:
        f.write(new_code)
    print("M-PLAYLIST [PATCH]: Premature double-toggle eradicated from fire_cue().")
elif re.search(fallback, code):
    new_code = re.sub(fallback, '', code)
    with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'w', encoding='utf-8') as f:
        f.write(new_code)
    print("M-PLAYLIST [PATCH]: Premature double-toggle (fallback) eradicated from fire_cue().")
elif re.search(fallback_2, code):
    new_code = re.sub(fallback_2, '', code)
    with open('Z:/M-Playlist/m_playlist/src/playlist.rs', 'w', encoding='utf-8') as f:
        f.write(new_code)
    print("M-PLAYLIST [PATCH]: Premature double-toggle (fallback_2) eradicated from fire_cue().")
else:
    print("M-PLAYLIST [PATCH]: Double-toggle not found in fire_cue().")
