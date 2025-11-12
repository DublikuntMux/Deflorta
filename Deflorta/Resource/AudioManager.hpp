#pragma once

#include <xaudio2.h>
#include <wrl/client.h>

#include <string>
#include <mutex>
#include <vector>
#include <atomic>
#include <memory>
#include <unordered_map>

struct AudioData
{
    std::vector<BYTE> buffer;
    WAVEFORMATEX format{};
};

struct MusicChannel
{
    IXAudio2SourceVoice* voice = nullptr;
    XAUDIO2_BUFFER xaBuffer{};
    bool active = false;
    float volume = 1.0f;
    std::vector<BYTE> buffer;

    ~MusicChannel()
    {
        if (voice)
        {
            voice->DestroyVoice();
            voice = nullptr;
        }
        buffer.clear();
    }
};

class AudioManager final
{
public:
    static bool Initialize();
    static void Uninitialize();

    static void PlaySfx(const std::string& id);
    static IXAudio2SourceVoice* PlaySfxAdvanced(const std::string& id, bool loop, float freqRatio = 1.0f,
                                                float volumeMul = 1.0f);
    static void StopVoice(IXAudio2SourceVoice* voice);

    static bool PreloadAudio(const std::string& id, const std::string& filePath);
    static void UnloadAudio(const std::string& id);

    static void SetSfxVolume(float volume);
    static float GetSfxVolume() noexcept { return sfxVolume_; }

private:
    static bool LoadOggFile(const std::string& filePath, AudioData& outData);
    static void PlayAudioData(const AudioData& data);

    static Microsoft::WRL::ComPtr<IXAudio2> xaudio_;
    static IXAudio2MasteringVoice* masterVoice_;

    static std::mutex audioMutex_;
    static std::atomic<float> sfxVolume_;
    static std::atomic<bool> running_;

    static std::unordered_map<std::string, AudioData> audioCache_;
};
