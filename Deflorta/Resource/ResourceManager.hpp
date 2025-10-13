#pragma once

#include <string>
#include <unordered_map>

#include "AudioManager.hpp"

class ResourceManager
{
public:
    static bool LoadManifest(const std::string& manifestPath);
    static bool LoadGroup(const std::string& groupName);

    static std::string GetAudio(const std::string& id);

private:
    struct ResourceEntry {
        std::string path;
        bool loaded = false;
    };

    struct DefaultSettings {
        std::string basePath;
        std::string idPrefix;
    };

    struct ResourceGroup {
        std::unordered_map<std::string, ResourceEntry> sounds;
        std::unordered_map<std::string, ResourceEntry> images;
        bool isLoaded = false;
    };

    static std::unordered_map<std::string, ResourceGroup> groups;
    static std::string resourceBasePath;
    static DefaultSettings currentDefaults;
};
