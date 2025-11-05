#pragma once

#include "../Render/IRenderBackend.hpp"
#include "../Render/PixelData.hpp"

#include <string>
#include <unordered_map>
#include <mutex>
#include <memory>

struct ResourceEntry
{
    std::string path;
    bool loaded = false;
    int rows = 1;
    int cols = 1;
};

struct FontEntry
{
    std::string path;
    std::wstring fontFamily;
    bool loaded = false;
};

struct DefaultSettings
{
    std::string basePath;
    std::string idPrefix;
};

struct ResourceGroup
{
    std::unordered_map<std::string, ResourceEntry> sounds;
    std::unordered_map<std::string, ResourceEntry> images;
    std::unordered_map<std::string, FontEntry> fonts;
    bool isLoaded = false;
};

class ResourceManager final
{
public:
    static void SetRenderBackend(IRenderBackend* backend);

    static bool LoadManifest(const std::string& manifestPath);
    static bool LoadGroup(const std::string& groupName);

    static std::shared_ptr<ITexture> GetImage(const std::string& id);
    static std::wstring GetFont(const std::string& id);

    static void PreloadAudio(const std::string& id);
    static bool PreloadReanimImage(const std::string& id);

private:
    static bool LoadPngFile(const std::string& filePath, PixelData& outData);
    static bool CreateTexture(const PixelData& data, std::shared_ptr<ITexture>* outTexture);
    static bool LoadFont(const std::string& id, const std::string& filePath, const std::wstring& familyName);
    static std::string TokenToReanimFileName(const std::string& id);
    static bool CreateSlicedImages(const std::string& baseId, const PixelData& sourceData, int rows, int cols);

    static IRenderBackend* backend_;
    static std::unordered_map<std::string, ResourceGroup> groups_;
    static std::unordered_map<std::string, std::shared_ptr<ITexture>> images_;
    static std::unordered_map<std::string, std::wstring> fonts_;

    static std::string resourceBasePath_;
    static DefaultSettings currentDefaults;

    static std::mutex groupsMutex_;
};
