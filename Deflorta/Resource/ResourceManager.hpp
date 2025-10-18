#pragma once

#include <string>
#include <unordered_map>
#include <mutex>

#include <d2d1.h>
#include <wrl/client.h>

#include "AudioManager.hpp"

struct ResourceEntry
{
    std::string path;
    bool loaded = false;
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

struct PngData
{
    std::vector<uint8_t> pixels;
    uint32_t width = 0;
    uint32_t height = 0;
};

class ResourceManager final
{
public:
    static bool LoadManifest(const std::string& manifestPath);
    static bool LoadGroup(const std::string& groupName);

    static std::string GetAudio(const std::string& id);
    static ID2D1Bitmap* GetImage(const std::string& id);
    static std::wstring GetFont(const std::string& id);

    static bool EnsureReanimImage(const std::string& id);

private:
    static bool LoadPngFile(const std::string& filePath, PngData& outData);
    static bool CreateD2DBitmap(const PngData& data, ID2D1Bitmap** outBitmap);
    static bool LoadFont(const std::string& id, const std::string& filePath, const std::wstring& familyName);
    static std::string TokenToReanimFileName(const std::string& id);

    static std::unordered_map<std::string, ResourceGroup> groups_;
    static std::unordered_map<std::string, Microsoft::WRL::ComPtr<ID2D1Bitmap>> images_;
    static std::unordered_map<std::string, std::wstring> fonts_;

    static std::string resourceBasePath_;
    static DefaultSettings currentDefaults;

    static std::mutex groupsMutex_;
};
