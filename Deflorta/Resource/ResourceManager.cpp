#include "ResourceManager.hpp"

#include <filesystem>
#include <ranges>

#include <pugixml.hpp>

std::unordered_map<std::string, ResourceManager::ResourceGroup> ResourceManager::groups;
std::wstring ResourceManager::resourceBasePath;
ResourceManager::DefaultSettings ResourceManager::currentDefaults;

bool ResourceManager::LoadManifest(const std::wstring& manifestPath)
{
    pugi::xml_document doc;
    if (!doc.load_file(manifestPath.c_str())) return false;

    resourceBasePath = std::filesystem::path(manifestPath).parent_path().wstring();

    auto root = doc.child("ResourceManifest");
    if (!root) return false;

    for (auto groupNode : root.children("Resources"))
    {
        std::string groupId = groupNode.attribute("id").as_string();
        auto& group = groups[groupId];

        currentDefaults = DefaultSettings();

        for (auto resNode : groupNode.children())
        {
            std::string nodeName = resNode.name();
            
            if (nodeName == "SetDefaults")
            {
                currentDefaults.basePath = std::filesystem::path(resNode.attribute("path").as_string()).wstring();
                currentDefaults.idPrefix = resNode.attribute("idprefix").as_string();
            }
            else if (nodeName == "Sound")
            {
                std::string id = resNode.attribute("id").as_string();
                std::string fullId = currentDefaults.idPrefix + id;
                
                ResourceEntry entry;
                entry.path = std::filesystem::path(currentDefaults.basePath) / 
                            (std::string(resNode.attribute("path").as_string()) + ".ogg");
                
                group.sounds[fullId] = entry;
            }
            else if (nodeName == "Image")
            {
                std::string id = resNode.attribute("id").as_string();
                std::string fullId = currentDefaults.idPrefix + id;
                
                ResourceEntry entry;
                entry.path = std::filesystem::path(currentDefaults.basePath) / 
                            std::string(resNode.attribute("path").as_string());
                
                group.images[fullId] = entry;
            }
        }
    }

    return true;
}

bool ResourceManager::LoadGroup(const std::string& groupName)
{
    const auto it = groups.find(groupName);
    if (it == groups.end()) return false;
    
    if (it->second.isLoaded) return true;

    for (auto& [id, entry] : it->second.sounds)
    {
        if (!entry.loaded)
        {
            std::wstring fullPath = std::filesystem::path(resourceBasePath) / entry.path;
            if (AudioManager::PreloadAudio(id, fullPath))
            {
                entry.loaded = true;
            }
        }
    }

    it->second.isLoaded = true;
    return true;
}

std::wstring ResourceManager::GetAudio(const std::string& id)
{
    for (auto& group : groups | std::views::values)
    {
        auto it = group.sounds.find(id);
        if (it != group.sounds.end())
        {
            if (!it->second.loaded)
            {
                const std::wstring fullPath = std::filesystem::path(resourceBasePath) / it->second.path;
                if (AudioManager::PreloadAudio(id, fullPath))
                {
                    it->second.loaded = true;
                }
            }
            AudioManager::PlaySfx(id);
            return std::filesystem::path(resourceBasePath) / it->second.path;
        }
    }
    return L"";
}
