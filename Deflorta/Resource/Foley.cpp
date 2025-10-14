#include "Foley.hpp"

#include <cmath>
#include <algorithm>

#include "ResourceManager.hpp"
#include "../Base/Random.hpp"

std::unordered_map<FoleyType, IXAudio2SourceVoice*> Foley::activeVoices_;
std::unordered_map<FoleyType, int> Foley::lastIndex_;
std::mutex Foley::mtx_;

void Foley::Play(FoleyType type)
{
    const auto it = foleyMap_.find(type);
    if (it == foleyMap_.end()) return;

    const FoleyParams& params = it->second;
    const FoleyFlags flags = params.foleyFlags;

    const bool loop = (flags & FoleyFlags::Loop) == FoleyFlags::Loop;
    const bool oneAtTime = (flags & FoleyFlags::OneAtTime) == FoleyFlags::OneAtTime;
    const bool dontRepeat = (flags & FoleyFlags::DontRepeat) == FoleyFlags::DontRepeat;

    std::lock_guard guard(mtx_);

    if (loop)
    {
        const auto vit = activeVoices_.find(type);
        if (vit != activeVoices_.end() && vit->second)
        {
            if (oneAtTime)
                return;
            return;
        }
    }

    if (params.sfxId.empty()) return;

    for (const auto& id : params.sfxId)
        (void)ResourceManager::GetAudio(id);

    int chosen = 0;
    if (params.sfxId.size() == 1)
    {
        chosen = 0;
    }
    else
    {
        const int size = static_cast<int>(params.sfxId.size());
        const int last = dontRepeat ? (lastIndex_.contains(type) ? lastIndex_[type] : -1) : -1;

        for (int attempts = 0; attempts < 4; ++attempts)
        {
            const int idx = Random::UniformInt(0, size - 1);
            if (idx != last)
            {
                chosen = idx;
                break;
            }
            if (attempts == 3) chosen = idx;
        }
    }

    lastIndex_[type] = chosen;
    const std::string& sfx = params.sfxId[static_cast<size_t>(chosen)];

    const float minSemi = std::min(0.0f, params.pitchRange);
    const float maxSemi = std::max(0.0f, params.pitchRange);
    float semis = 0.0f;
    if (minSemi != maxSemi)
    {
        semis = Random::UniformFloat(minSemi, maxSemi);
    }
    const float ratio = std::clamp(SemitonesToRatio(semis), 0.5f, 2.0f);

    IXAudio2SourceVoice* voice = AudioManager::PlaySfxAdvanced(sfx, loop, ratio, 1.0f);

    if (loop)
    {
        if (voice)
        {
            activeVoices_[type] = voice;
        }
    }
}

void Foley::Stop(FoleyType type)
{
    std::lock_guard guard(mtx_);
    const auto it = activeVoices_.find(type);
    if (it != activeVoices_.end() && it->second)
    {
        AudioManager::StopVoice(it->second);
        it->second = nullptr;
        activeVoices_.erase(it);
    }
}

float Foley::SemitonesToRatio(float semitones)
{
    return std::pow(2.0f, semitones / 12.0f);
}
