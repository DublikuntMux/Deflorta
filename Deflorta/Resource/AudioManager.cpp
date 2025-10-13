#include "AudioManager.hpp"

#include <algorithm>
#include <chrono>
#include <filesystem>
#include <thread>

#include <vorbis/vorbisfile.h>

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
std::unordered_map<std::string, AudioManager::AudioData> AudioManager::audioCache;

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

bool AudioManager::LoadOggFile(const std::wstring& filePath, AudioData& outData)
{
    FILE* file = nullptr;
    _wfopen_s(&file, filePath.c_str(), L"rb");
    if (!file) return false;

    OggVorbis_File vf;
    if (ov_open_callbacks(file, &vf, nullptr, 0, OV_CALLBACKS_DEFAULT) < 0)
    {
        fclose(file);
        return false;
    }

    vorbis_info* vi = ov_info(&vf, -1);
    if (!vi)
    {
        ov_clear(&vf);
        return false;
    }

    WAVEFORMATEX& wfx = outData.format;
    wfx.wFormatTag = WAVE_FORMAT_PCM;
    wfx.nChannels = static_cast<WORD>(vi->channels);
    wfx.nSamplesPerSec = static_cast<DWORD>(vi->rate);
    wfx.wBitsPerSample = 16;
    wfx.nBlockAlign = wfx.nChannels * (wfx.wBitsPerSample / 8);
    wfx.nAvgBytesPerSec = wfx.nSamplesPerSec * wfx.nBlockAlign;
    wfx.cbSize = 0;

    constexpr size_t BUFFER_SIZE = 4096;
    std::vector<char> temp(BUFFER_SIZE);
    outData.buffer.clear();

    int bitstream = 0;
    long bytesRead;
    do
    {
        bytesRead = ov_read(&vf, temp.data(), static_cast<int>(temp.size()), 0, 2, 1, &bitstream);
        if (bytesRead > 0)
            outData.buffer.insert(outData.buffer.end(), temp.begin(), temp.begin() + bytesRead);
    } while (bytesRead > 0);

    ov_clear(&vf);
    return !outData.buffer.empty();
}

void AudioManager::PlayMusic(const std::wstring& filePath, float fadeTimeSec)
{
    std::lock_guard lock(audioMutex);

    AudioData data;
    if (!LoadOggFile(filePath, data)) return;

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

void AudioManager::PlaySfx(const std::string& id)
{
    std::lock_guard lock(audioMutex);
    const auto it = audioCache.find(id);
    if (it != audioCache.end())
    {
        PlayAudioData(it->second);
    }
}

void AudioManager::PlaySfxByPath(const std::wstring& filePath)
{
    AudioData data;
    if (!LoadOggFile(filePath, data)) return;
    PlayAudioData(data);
}

bool AudioManager::PreloadAudio(const std::string& id, const std::wstring& filePath)
{
    AudioData data;
    if (!LoadOggFile(filePath, data)) return false;

    std::lock_guard lock(audioMutex);
    audioCache[id] = std::move(data);
    return true;
}

void AudioManager::UnloadAudio(const std::string& id)
{
    std::lock_guard lock(audioMutex);
    audioCache.erase(id);
}

void AudioManager::PlayAudioData(const AudioData& data)
{
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
        } while (state.BuffersQueued > 0);
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
