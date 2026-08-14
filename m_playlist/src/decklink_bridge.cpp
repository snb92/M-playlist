#include <windows.h>
#include <stdint.h>
#include "DeckLinkAPI_h.h"

class DeckLinkCaptureCallback : public IDeckLinkInputCallback {
    ULONG m_refCount = 1;
    void* m_userData;
    void (*m_rustCb)(void*, void*, const uint8_t*, uint32_t, uint32_t, uint32_t);
public:
    DeckLinkCaptureCallback(void* userData, void (*cb)(void*, void*, const uint8_t*, uint32_t, uint32_t, uint32_t)) : m_userData(userData), m_rustCb(cb) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, LPVOID *ppv) { return E_NOINTERFACE; }
    ULONG STDMETHODCALLTYPE AddRef() { return InterlockedIncrement(&m_refCount); }
    ULONG STDMETHODCALLTYPE Release() { ULONG c = InterlockedDecrement(&m_refCount); if (c == 0) delete this; return c; }
    HRESULT STDMETHODCALLTYPE VideoInputFormatChanged(BMDVideoInputFormatChangedEvents ev, IDeckLinkDisplayMode* mode, BMDDisplayModeFlags flags) { return S_OK; }
    
    HRESULT STDMETHODCALLTYPE VideoInputFrameArrived(IDeckLinkVideoInputFrame* videoFrame, IDeckLinkAudioInputPacket* audioPacket) {
        if (videoFrame && (videoFrame->GetFlags() & bmdFrameHasNoInputSource) == 0) {
            videoFrame->AddRef();
            uint8_t* data = nullptr;
            videoFrame->GetBytes((void**)&data);
            m_rustCb(m_userData, (void*)videoFrame, data, videoFrame->GetWidth(), videoFrame->GetHeight(), videoFrame->GetRowBytes());
        }
        return S_OK;
    }
};

extern "C" {
    void* decklink_start(uint8_t hardware_index, void (*cb)(void*, void*, const uint8_t*, uint32_t, uint32_t, uint32_t), void* user_data) {
        CoInitializeEx(NULL, COINIT_MULTITHREADED);
        IDeckLinkIterator* iterator = nullptr;
        if (FAILED(CoCreateInstance(CLSID_CDeckLinkIterator, NULL, CLSCTX_ALL, IID_IDeckLinkIterator, (void**)&iterator))) return nullptr;
        
        IDeckLink* decklink = nullptr;
        for (uint8_t i = 0; i <= hardware_index; i++) {
            if (decklink) { decklink->Release(); decklink = nullptr; }
            if (iterator->Next(&decklink) != S_OK) break;
        }
        iterator->Release();
        if (!decklink) return nullptr;

        IDeckLinkInput* input = nullptr;
        decklink->QueryInterface(IID_IDeckLinkInput, (void**)&input);
        decklink->Release();
        if (!input) return nullptr;

        DeckLinkCaptureCallback* callback = new DeckLinkCaptureCallback(user_data, cb);
        input->SetCallback(callback);
        input->EnableVideoInput(bmdModeHD1080i5994, bmdFormat8BitYUV, bmdVideoInputFlagDefault);
        input->StartStreams();
        callback->Release();
        return input;
    }

    void decklink_release_frame(void* frame) { if (frame) ((IDeckLinkVideoInputFrame*)frame)->Release(); }
    
    void decklink_stop(void* ctx) {
        if (ctx) {
            IDeckLinkInput* input = (IDeckLinkInput*)ctx;
            input->StopStreams(); input->DisableVideoInput(); input->Release();
        }
    }
}
