#pragma once
#include <windows.h>
#include <unknwn.h>

typedef uint32_t BMDVideoInputFormatChangedEvents;
typedef uint32_t BMDDisplayModeFlags;
typedef uint32_t BMDVideoInputFlags;
typedef uint32_t BMDPixelFormat;
typedef uint32_t BMDDisplayMode;
typedef uint32_t BMDAudioSampleRate;
typedef uint32_t BMDAudioSampleType;

const uint32_t bmdFrameHasNoInputSource = 1;
const uint32_t bmdVideoInputFlagDefault = 0;
const uint32_t bmdFormat8BitYUV = 0x32767579;
const uint32_t bmdAudioSampleRate48kHz = 48000;
const uint32_t bmdAudioSampleType16bitInteger = 16;
const uint32_t bmdModeHD1080i50 = 0x68693530;
const uint32_t bmdModeHD1080i5994 = 0x68693539;

EXTERN_C const IID IID_IDeckLinkInputCallback;
EXTERN_C const IID IID_IDeckLinkDisplayMode;
EXTERN_C const IID IID_IDeckLinkVideoInputFrame;
EXTERN_C const IID IID_IDeckLinkAudioInputPacket;
EXTERN_C const IID IID_IDeckLinkIterator;
EXTERN_C const IID IID_IDeckLink;
EXTERN_C const IID IID_IDeckLinkInput;
EXTERN_C const CLSID CLSID_CDeckLinkIterator;

MIDL_INTERFACE("11111111-1111-1111-1111-111111111111")
IDeckLinkDisplayMode : public IUnknown {
};

MIDL_INTERFACE("22222222-2222-2222-2222-222222222222")
IDeckLinkAudioInputPacket : public IUnknown {
};

MIDL_INTERFACE("33333333-3333-3333-3333-333333333333")
IDeckLinkVideoInputFrame : public IUnknown {
public:
    virtual long STDMETHODCALLTYPE GetWidth(void) = 0;
    virtual long STDMETHODCALLTYPE GetHeight(void) = 0;
    virtual long STDMETHODCALLTYPE GetRowBytes(void) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetBytes(void **buffer) = 0;
    virtual BMDVideoInputFlags STDMETHODCALLTYPE GetFlags(void) = 0;
};

MIDL_INTERFACE("44444444-4444-4444-4444-444444444444")
IDeckLinkInputCallback : public IUnknown {
public:
    virtual HRESULT STDMETHODCALLTYPE VideoInputFormatChanged(BMDVideoInputFormatChangedEvents, IDeckLinkDisplayMode*, BMDDisplayModeFlags) = 0;
    virtual HRESULT STDMETHODCALLTYPE VideoInputFrameArrived(IDeckLinkVideoInputFrame*, IDeckLinkAudioInputPacket*) = 0;
};

MIDL_INTERFACE("55555555-5555-5555-5555-555555555555")
IDeckLink : public IUnknown {
};

MIDL_INTERFACE("66666666-6666-6666-6666-666666666666")
IDeckLinkIterator : public IUnknown {
public:
    virtual HRESULT STDMETHODCALLTYPE Next(IDeckLink **) = 0;
};

MIDL_INTERFACE("77777777-7777-7777-7777-777777777777")
IDeckLinkInput : public IUnknown {
public:
    virtual HRESULT STDMETHODCALLTYPE SetCallback(IDeckLinkInputCallback *theCallback) = 0;
    virtual HRESULT STDMETHODCALLTYPE EnableVideoInput(BMDDisplayMode displayMode, BMDPixelFormat pixelFormat, BMDVideoInputFlags flags) = 0;
    virtual HRESULT STDMETHODCALLTYPE DisableVideoInput(void) = 0;
    virtual HRESULT STDMETHODCALLTYPE EnableAudioInput(BMDAudioSampleRate sampleRate, BMDAudioSampleType sampleType, uint32_t channelCount) = 0;
    virtual HRESULT STDMETHODCALLTYPE DisableAudioInput(void) = 0;
    virtual HRESULT STDMETHODCALLTYPE StartStreams(void) = 0;
    virtual HRESULT STDMETHODCALLTYPE StopStreams(void) = 0;
};
