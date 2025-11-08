#include "Foley.hpp"

#include "ResourceManager.hpp"
#include "AudioManager.hpp"
#include "../Base/Random.hpp"
#include "../Utils.hpp"

#include <pugixml.hpp>

#include <cmath>
#include <algorithm>
#include <iostream>
#include <sstream>
#include <filesystem>

std::unordered_map<std::string, FoleyParams> Foley::foleyMap_;
std::unordered_map<std::string, IXAudio2SourceVoice*> Foley::activeVoices_;
std::unordered_map<std::string, int> Foley::lastIndex_;
std::mutex Foley::mtx_;

void Foley::Play(const std::string& name)
{
    const auto it = foleyMap_.find(name);
    if (it == foleyMap_.end())
    {
        std::cerr << "Foley::Play - Error: Sound '" << name << "' not found in foley definitions\n";
        return;
    }

    const FoleyParams& params = it->second;
    const FoleyFlags flags = params.foleyFlags;

    const bool loop = (flags & FoleyFlags::Loop) == FoleyFlags::Loop;
    const bool oneAtTime = (flags & FoleyFlags::OneAtTime) == FoleyFlags::OneAtTime;
    const bool dontRepeat = (flags & FoleyFlags::DontRepeat) == FoleyFlags::DontRepeat;

    std::lock_guard guard(mtx_);

    if (loop)
    {
        const auto vit = activeVoices_.find(name);
        if (vit != activeVoices_.end() && vit->second)
        {
            if (oneAtTime)
                return;
            return;
        }
    }

    if (params.sfxId.empty())
    {
        std::cerr << "Foley::Play - Error: Sound '" << name << "' has no sfx entries\n";
        return;
    }

    for (const auto& id : params.sfxId)
        ResourceManager::PreloadAudio(id);

    int chosen = 0;
    if (params.sfxId.size() == 1)
    {
        chosen = 0;
    }
    else
    {
        const int size = static_cast<int>(params.sfxId.size());
        const int last = dontRepeat ? (lastIndex_.contains(name) ? lastIndex_[name] : -1) : -1;

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

    lastIndex_[name] = chosen;
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
            activeVoices_[name] = voice;
        }
    }
}

void Foley::Stop(const std::string& name)
{
    std::lock_guard guard(mtx_);
    const auto it = activeVoices_.find(name);
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

FoleyFlags Foley::ParseFlags(const std::string& flagsStr)
{
    auto result = FoleyFlags::None;

    if (flagsStr.empty() || flagsStr == "None")
        return result;

    std::istringstream ss(flagsStr);
    std::string flag;

    while (std::getline(ss, flag, '|'))
    {
        flag.erase(0, flag.find_first_not_of(" \t\r\n"));
        flag.erase(flag.find_last_not_of(" \t\r\n") + 1);

        if (flag == "Loop")
            result |= FoleyFlags::Loop;
        else if (flag == "OneAtTime")
            result |= FoleyFlags::OneAtTime;
        else if (flag == "MuteOnPause")
            result |= FoleyFlags::MuteOnPause;
        else if (flag == "MusicVolume")
            result |= FoleyFlags::MusicVolume;
        else if (flag == "DontRepeat")
            result |= FoleyFlags::DontRepeat;
    }

    return result;
}

void Foley::LoadManifest()
{
    const std::string exeDir = Utils::GetExecutableDir();
    std::string path = (std::filesystem::path(exeDir) / "resources" / "foley.xml").string();

    pugi::xml_document doc;
    const pugi::xml_parse_result result = doc.load_file(path.c_str());
    if (!result)
    {
        std::cerr << "Foley::LoadManifest - Error: Failed to load XML file '" << path
            << "' - " << result.description() << " at offset " << result.offset << "\n";
        return;
    }

    const pugi::xml_node root = doc.child("foley");
    if (!root)
    {
        std::cerr << "Foley::LoadManifest - Error: Missing <foley> root element in '" << path << "'\n";
        return;
    }

    foleyMap_.clear();

    int loadedCount = 0;

    for (auto soundNode : root.children("sound"))
    {
        const std::string name = soundNode.attribute("name").as_string();
        if (name.empty())
        {
            std::cerr << "Foley::LoadManifest - Warning: Found <sound> element without 'name' attribute, skipping\n";
            continue;
        }

        FoleyParams params;

        if (const auto flagsNode = soundNode.child("flags"))
            params.foleyFlags = ParseFlags(flagsNode.text().as_string());
        else
            params.foleyFlags = FoleyFlags::None;

        if (const auto pitchNode = soundNode.child("pitchRange"))
            params.pitchRange = pitchNode.text().as_float(0.0f);
        else
            params.pitchRange = 0.0f;

        for (auto sfxNode : soundNode.children("sfx"))
        {
            if (std::string sfxId = sfxNode.text().as_string(); !sfxId.empty())
                params.sfxId.push_back(std::move(sfxId));
        }

        if (params.sfxId.empty())
        {
            std::cerr << "Foley::LoadManifest - Warning: Sound '" << name << "' has no <sfx> entries, skipping\n";
            continue;
        }

        foleyMap_[name] = std::move(params);
        loadedCount++;
    }
}
