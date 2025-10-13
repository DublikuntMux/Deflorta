#pragma once

#include <string>
#include <mutex>
#include <vector>
#include <atomic>
#include <memory>
#include <thread>
#include <unordered_map>

#include <xaudio2.h>
#include <wrl/client.h>

class AudioManager final
{
public:
    static bool Initialize();
    static void Uninitialize();

    static void PlayMusic(const std::wstring& filePath, float fadeTimeSec = 1.0f);
    static void StopMusic(float fadeTimeSec = 1.0f);

    static void PlaySfx(const std::string& id);
    static void PlaySfxByPath(const std::wstring& filePath);
    static bool PreloadAudio(const std::string& id, const std::wstring& filePath);
    static void UnloadAudio(const std::string& id);

    static void SetMasterVolume(float volume);
    static float GetMasterVolume() noexcept { return masterVolume; }

    static void SetSfxVolume(float volume);
    static float GetSfxVolume() noexcept { return sfxVolume; }

private:
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

    static bool LoadOggFile(const std::wstring& filePath, AudioData& outData);
    static void FadeThreadFunc(float duration, bool stopAfter);
    static void PlayAudioData(const AudioData& data);

    static Microsoft::WRL::ComPtr<IXAudio2> xaudio;
    static IXAudio2MasteringVoice* masterVoice;

    static MusicChannel musicA;
    static MusicChannel musicB;
    static bool usingA;

    static std::mutex audioMutex;
    static std::atomic<float> masterVolume;
    static std::atomic<float> sfxVolume;
    static std::atomic<bool> running;
    static std::thread fadeThread;

    static std::unordered_map<std::string, AudioData> audioCache;
};
