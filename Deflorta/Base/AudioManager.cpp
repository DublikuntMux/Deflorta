#include "AudioManager.hpp"

#include <algorithm>
#include <fstream>
#include <chrono>
#include <thread>

Microsoft::WRL::ComPtr<IXAudio2> AudioManager::xaudio;
IXAudio2MasteringVoice* AudioManager::masterVoice = nullptr;

AudioManager::MusicChannel AudioManager::musicA;
AudioManager::MusicChannel AudioManager::musicB;
bool AudioManager::usingA = true;

std::mutex AudioManager::audioMutex;
std::atomic<float> AudioManager::masterVolume = 1.0f;
std::atomic<float> AudioManager::sfxVolume = 1.0f;
std::atomic<bool> AudioManager::running = true;
std::thread AudioManager::fadeThread;

bool AudioManager::Initialize()
{
    if (FAILED(XAudio2Create(&xaudio, 0, XAUDIO2_DEFAULT_PROCESSOR)))
        return false;

    if (FAILED(xaudio->CreateMasteringVoice(&masterVoice)))
        return false;

    return true;
}

void AudioManager::Uninitialize()
{
    running = false;
    if (fadeThread.joinable()) fadeThread.join();
    if (masterVoice) masterVoice->DestroyVoice();
}

bool AudioManager::LoadWaveFile(const std::wstring& filePath, AudioData& outData)
{
    std::ifstream file(filePath, std::ios::binary);
    if (!file) return false;

    DWORD chunkType;
    DWORD chunkSize;
    file.read(reinterpret_cast<char*>(&chunkType), 4);
    file.seekg(12, std::ios::beg);

    WAVEFORMATEX wfx{};
    DWORD dataSize = 0;

    while (file.read(reinterpret_cast<char*>(&chunkType), 4))
    {
        file.read(reinterpret_cast<char*>(&chunkSize), 4);
        if (chunkType == ' tmf')
        {
            // "fmt "
            file.read(reinterpret_cast<char*>(&wfx), chunkSize);
        }
        else if (chunkType == 'atad')
        {
            // "data"
            outData.buffer.resize(chunkSize);
            file.read(reinterpret_cast<char*>(outData.buffer.data()), chunkSize);
            dataSize = chunkSize;
        }
        else
        {
            file.seekg(chunkSize, std::ios::cur);
        }
        if (file.eof()) break;
    }

    outData.format = wfx;
    return dataSize > 0;
}

void AudioManager::PlayMusic(const std::wstring& filePath, float fadeTimeSec)
{
    std::lock_guard lock(audioMutex);

    AudioData data;
    if (!LoadWaveFile(filePath, data)) return;

    auto& next = usingA ? musicB : musicA;
    next.buffer = std::move(data.buffer);

    if (next.voice) next.voice->DestroyVoice();

    if (FAILED(xaudio->CreateSourceVoice(&next.voice, &data.format)))
        return;

    next.xaBuffer = {};
    next.xaBuffer.AudioBytes = static_cast<UINT32>(next.buffer.size());
    next.xaBuffer.pAudioData = next.buffer.data();
    next.xaBuffer.Flags = XAUDIO2_END_OF_STREAM;
    next.xaBuffer.LoopCount = XAUDIO2_LOOP_INFINITE;

    next.voice->SubmitSourceBuffer(&next.xaBuffer);
    next.voice->SetVolume(0.0f * masterVolume);
    next.voice->Start();

    usingA = !usingA;

    if (fadeThread.joinable()) fadeThread.join();
    fadeThread = std::thread(&AudioManager::FadeThreadFunc, fadeTimeSec, false);
}

void AudioManager::StopMusic(float fadeTimeSec)
{
    if (fadeThread.joinable()) fadeThread.join();
    fadeThread = std::thread(&AudioManager::FadeThreadFunc, fadeTimeSec, true);
}

void AudioManager::FadeThreadFunc(float duration, bool stopAfter)
{
    const float step = 1.0f / std::max(duration * 60.0f, 1.0f);

    float t = 0.0f;
    while (t < 1.0f && running)
    {
        t += step;
        {
            const float fade = std::clamp(t, 0.0f, 1.0f);
            std::lock_guard lock(audioMutex);
            if (musicA.voice)
                musicA.voice->SetVolume((usingA ? fade : 1.0f - fade) * masterVolume);
            if (musicB.voice)
                musicB.voice->SetVolume((usingA ? 1.0f - fade : fade) * masterVolume);
        }

        std::this_thread::sleep_for(std::chrono::milliseconds(16));
    }

    if (stopAfter)
    {
        std::lock_guard lock(audioMutex);
        if (musicA.voice) musicA.voice->Stop();
        if (musicB.voice) musicB.voice->Stop();
    }
}

void AudioManager::PlaySfx(const std::wstring& filePath)
{
    AudioData data;
    if (!LoadWaveFile(filePath, data)) return;

    IXAudio2SourceVoice* voice = nullptr;
    if (FAILED(xaudio->CreateSourceVoice(&voice, &data.format))) return;

    XAUDIO2_BUFFER buffer = {};
    buffer.AudioBytes = static_cast<UINT32>(data.buffer.size());
    buffer.pAudioData = data.buffer.data();
    buffer.Flags = XAUDIO2_END_OF_STREAM;

    voice->SubmitSourceBuffer(&buffer);
    voice->SetVolume(sfxVolume);
    voice->Start();

    std::thread([voice]
    {
        XAUDIO2_VOICE_STATE state;
        do
        {
            voice->GetState(&state);
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        while (state.BuffersQueued > 0);
        voice->DestroyVoice();
    }).detach();
}

void AudioManager::SetMasterVolume(float volume)
{
    masterVolume = std::clamp(volume, 0.0f, 1.0f);
    if (masterVoice) masterVoice->SetVolume(masterVolume);
}

void AudioManager::SetSfxVolume(float volume)
{
    sfxVolume = std::clamp(volume, 0.0f, 1.0f);
}
