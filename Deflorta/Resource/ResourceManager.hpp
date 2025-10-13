#pragma once

#include <string>
#include <unordered_map>

#include "AudioManager.hpp"

class ResourceManager
{
public:
    static bool LoadManifest(const std::wstring& manifestPath);
    static bool LoadGroup(const std::string& groupName);

    static std::wstring GetAudio(const std::string& id);

private:
    struct ResourceEntry {
        std::wstring path;
        bool loaded = false;
    };

    struct DefaultSettings {
        std::wstring basePath;
        std::string idPrefix;
    };

    struct ResourceGroup {
        std::unordered_map<std::string, ResourceEntry> sounds;
        std::unordered_map<std::string, ResourceEntry> images;
        bool isLoaded = false;
    };

    static std::unordered_map<std::string, ResourceGroup> groups;
    static std::wstring resourceBasePath;
    static DefaultSettings currentDefaults;
};
