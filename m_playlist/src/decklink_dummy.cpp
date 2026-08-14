#include <stdint.h>
extern "C" {
    void* decklink_start(uint8_t idx, void (*cb)(void*, void*, const uint8_t*, uint32_t, uint32_t, uint32_t), void* u) { return nullptr; }
    void decklink_release_frame(void* frame) {}
    void decklink_stop(void* ctx) {}
}
