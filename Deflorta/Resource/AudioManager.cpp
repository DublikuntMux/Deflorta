#include "AudioManager.hpp"

#include <algorithm>
#include <chrono>
#include <filesystem>
#include <thread>

#include <vorbis/vorbisfile.h>

Microsoft::WRL::ComPtr<IXAudio2> AudioManager::xaudio_;
IXAudio2MasteringVoice* AudioManager::masterVoice_ = nullptr;

MusicChannel AudioManager::musicA_;
MusicChannel AudioManager::musicB_;
bool AudioManager::usingA_ = true;

std::mutex AudioManager::audioMutex_;
std::atomic<float> AudioManager::masterVolume_ = 1.0f;
std::atomic<float> AudioManager::sfxVolume_ = 1.0f;
std::atomic<bool> AudioManager::running_ = true;
std::thread AudioManager::fadeThread_;
std::unordered_map<std::string, AudioData> AudioManager::audioCache_;

bool AudioManager::Initialize()
{
    if (FAILED(XAudio2Create(&xaudio_, 0, XAUDIO2_DEFAULT_PROCESSOR)))
        return false;

    if (FAILED(xaudio_->CreateMasteringVoice(&masterVoice_)))
        return false;

    return true;
}

void AudioManager::Uninitialize()
{
    running_ = false;
    if (fadeThread_.joinable()) fadeThread_.join();
    if (masterVoice_) masterVoice_->DestroyVoice();
}

bool AudioManager::LoadOggFile(const std::string& filePath, AudioData& outData)
{
    FILE* file = nullptr;
    fopen_s(&file, filePath.c_str(), "rb");
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
    }
    while (bytesRead > 0);

    ov_clear(&vf);
    return !outData.buffer.empty();
}

void AudioManager::PlayMusic(const std::string& filePath, float fadeTimeSec)
{
    std::lock_guard lock(audioMutex_);

    AudioData data;
    if (!LoadOggFile(filePath, data)) return;

    auto& next = usingA_ ? musicB_ : musicA_;
    next.buffer = std::move(data.buffer);

    if (next.voice) next.voice->DestroyVoice();

    if (FAILED(xaudio_->CreateSourceVoice(&next.voice, &data.format)))
        return;

    next.xaBuffer = {};
    next.xaBuffer.AudioBytes = static_cast<UINT32>(next.buffer.size());
    next.xaBuffer.pAudioData = next.buffer.data();
    next.xaBuffer.Flags = XAUDIO2_END_OF_STREAM;
    next.xaBuffer.LoopCount = XAUDIO2_LOOP_INFINITE;

    next.voice->SubmitSourceBuffer(&next.xaBuffer);
    next.voice->SetVolume(0.0f * masterVolume_);
    next.voice->Start();

    usingA_ = !usingA_;

    if (fadeThread_.joinable()) fadeThread_.join();
    fadeThread_ = std::thread(&AudioManager::FadeThreadFunc, fadeTimeSec, false);
}

void AudioManager::StopMusic(float fadeTimeSec)
{
    if (fadeThread_.joinable()) fadeThread_.join();
    fadeThread_ = std::thread(&AudioManager::FadeThreadFunc, fadeTimeSec, true);
}

void AudioManager::FadeThreadFunc(float duration, bool stopAfter)
{
    const float step = 1.0f / std::max(duration * 60.0f, 1.0f);

    float t = 0.0f;
    while (t < 1.0f && running_)
    {
        t += step;
        {
            const float fade = std::clamp(t, 0.0f, 1.0f);
            std::lock_guard lock(audioMutex_);
            if (musicA_.voice)
                musicA_.voice->SetVolume((usingA_ ? fade : 1.0f - fade) * masterVolume_);
            if (musicB_.voice)
                musicB_.voice->SetVolume((usingA_ ? 1.0f - fade : fade) * masterVolume_);
        }

        std::this_thread::sleep_for(std::chrono::milliseconds(16));
    }

    if (stopAfter)
    {
        std::lock_guard lock(audioMutex_);
        if (musicA_.voice) musicA_.voice->Stop();
        if (musicB_.voice) musicB_.voice->Stop();
    }
}

void AudioManager::PlaySfx(const std::string& id)
{
    std::lock_guard lock(audioMutex_);
    const auto it = audioCache_.find(id);
    if (it != audioCache_.end())
    {
        PlayAudioData(it->second);
    }
}

IXAudio2SourceVoice* AudioManager::PlaySfxAdvanced(const std::string& id, bool loop, float freqRatio, float volumeMul)
{
    std::lock_guard lock(audioMutex_);
    const auto it = audioCache_.find(id);
    if (it == audioCache_.end()) return nullptr;

    const AudioData& data = it->second;

    IXAudio2SourceVoice* voice = nullptr;
    if (FAILED(xaudio_->CreateSourceVoice(&voice, &data.format))) return nullptr;

    XAUDIO2_BUFFER buffer = {};
    buffer.AudioBytes = static_cast<UINT32>(data.buffer.size());
    buffer.pAudioData = data.buffer.data();
    if (loop)
    {
        buffer.LoopCount = XAUDIO2_LOOP_INFINITE;
        buffer.Flags = 0;
        buffer.LoopBegin = 0;
        buffer.LoopLength = 0;
    }
    else
    {
        buffer.Flags = XAUDIO2_END_OF_STREAM;
    }

    if (FAILED(voice->SubmitSourceBuffer(&buffer)))
    {
        voice->DestroyVoice();
        return nullptr;
    }

    const float vol = std::clamp(volumeMul, 0.0f, 1.0f) * sfxVolume_;
    voice->SetVolume(vol);

    if (freqRatio > 0.0f)
    {
        const float clamped = std::clamp(freqRatio, 0.5f, 2.0f);
        voice->SetFrequencyRatio(clamped);
    }

    voice->Start();

    if (!loop)
    {
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
        return nullptr;
    }

    return voice;
}

void AudioManager::StopVoice(IXAudio2SourceVoice* voice)
{
    if (!voice) return;
    voice->Stop();
    voice->DestroyVoice();
}

bool AudioManager::PreloadAudio(const std::string& id, const std::string& filePath)
{
    AudioData data;
    if (!LoadOggFile(filePath, data)) return false;

    std::lock_guard lock(audioMutex_);
    audioCache_[id] = std::move(data);
    return true;
}

void AudioManager::UnloadAudio(const std::string& id)
{
    std::lock_guard lock(audioMutex_);
    audioCache_.erase(id);
}

void AudioManager::PlayAudioData(const AudioData& data)
{
    IXAudio2SourceVoice* voice = nullptr;
    if (FAILED(xaudio_->CreateSourceVoice(&voice, &data.format))) return;

    XAUDIO2_BUFFER buffer = {};
    buffer.AudioBytes = static_cast<UINT32>(data.buffer.size());
    buffer.pAudioData = data.buffer.data();
    buffer.Flags = XAUDIO2_END_OF_STREAM;

    voice->SubmitSourceBuffer(&buffer);
    voice->SetVolume(sfxVolume_);
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
    masterVolume_ = std::clamp(volume, 0.0f, 1.0f);
    if (masterVoice_) masterVoice_->SetVolume(masterVolume_);
}

void AudioManager::SetSfxVolume(float volume)
{
    sfxVolume_ = std::clamp(volume, 0.0f, 1.0f);
}
