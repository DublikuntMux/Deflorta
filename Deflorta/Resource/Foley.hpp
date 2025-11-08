#pragma once

#include <xaudio2.h>

#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>
#include <mutex>

enum class FoleyFlags: std::uint8_t
{
    None = 0,
    Loop = 1 << 0,
    OneAtTime = 1 << 1,
    MuteOnPause = 1 << 2,
    MusicVolume = 1 << 3,
    DontRepeat = 1 << 4,
};

inline FoleyFlags operator|(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) | static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator&(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) & static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator^(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) ^ static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator~(FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        ~static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags& operator|=(FoleyFlags& lhs, FoleyFlags rhs)
{
    lhs = lhs | rhs;
    return lhs;
}

inline FoleyFlags& operator&=(FoleyFlags& lhs, FoleyFlags rhs)
{
    lhs = lhs & rhs;
    return lhs;
}

struct FoleyParams
{
    FoleyFlags foleyFlags;
    float pitchRange;
    std::vector<std::string> sfxId;
};

class Foley
{
public:
    static void LoadManifest();

    static void Play(const std::string& name);
    static void Stop(const std::string& name);

private:
    static float SemitonesToRatio(float semitones);
    static FoleyFlags ParseFlags(const std::string& flagsStr);

    static std::unordered_map<std::string, FoleyParams> foleyMap_;
    static std::unordered_map<std::string, IXAudio2SourceVoice*> activeVoices_;
    static std::unordered_map<std::string, int> lastIndex_;
    static std::mutex mtx_;
};
