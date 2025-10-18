#include "ResourceManager.hpp"

#include <filesystem>
#include <iostream>
#include <ranges>
#include <vector>

#include <pugixml.hpp>
#include <png.h>
#include <Windows.h>

#include "../Render/Renderer.hpp"

std::unordered_map<std::string, ResourceGroup> ResourceManager::groups_;
std::unordered_map<std::string, Microsoft::WRL::ComPtr<ID2D1Bitmap>> ResourceManager::images_;
std::unordered_map<std::string, std::wstring> ResourceManager::fonts_;
std::string ResourceManager::resourceBasePath_;
DefaultSettings ResourceManager::currentDefaults;
std::mutex ResourceManager::groupsMutex_;

bool ResourceManager::LoadFont(const std::string& id, const std::string& filePath, const std::wstring& familyName)
{
    const std::wstring wpath = std::filesystem::path(filePath).wstring();
    const int added = AddFontResourceExW(wpath.c_str(), FR_PRIVATE, nullptr);
    if (added <= 0)
        return false;

    fonts_[id] = familyName;
    return true;
}

std::wstring ResourceManager::GetFont(const std::string& id)
{
    std::lock_guard lock(groupsMutex_);
    auto it = fonts_.find(id);
    if (it == fonts_.end()) return L"";
    return it->second;
}

bool ResourceManager::EnsureReanimImage(const std::string& id)
{
    {
        std::lock_guard lock(groupsMutex_);
        for (auto& group : groups_ | std::views::values)
        {
            if (auto it = group.images.find(id); it != group.images.end())
            {
                if (!it->second.loaded)
                {
                    const std::string fullPath = (std::filesystem::path(resourceBasePath_) / it->second.path).string() +
                        ".png";
                    PngData pngData;
                    if (LoadPngFile(fullPath, pngData))
                    {
                        ID2D1Bitmap* bitmap = nullptr;
                        if (CreateD2DBitmap(pngData, &bitmap))
                        {
                            images_[id].Attach(bitmap);
                            it->second.loaded = true;
                        }
                    }
                }
                return images_.contains(id);
            }
        }
    }

    {
        std::lock_guard lock(groupsMutex_);
        auto& group = groups_["PreloadReanim"];
        auto& entry = group.images[id];
        if (!entry.loaded)
        {
            const std::string fileNameNoExt = TokenToReanimFileName(id);
            entry.path = (std::filesystem::path("reanim") / (fileNameNoExt)).string();

            const std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string() + ".png";
            PngData pngData;
            if (LoadPngFile(fullPath, pngData))
            {
                ID2D1Bitmap* bitmap = nullptr;
                if (CreateD2DBitmap(pngData, &bitmap))
                {
                    images_[id].Attach(bitmap);
                    entry.loaded = true;
                }
                else
                {
                    std::cout << "Error while create image: " << id << "\n";
                }
            }
            else
            {
                std::cout << "Error while load reanim image: " << fullPath << " for id " << id << "\n";
            }
        }
        return images_.contains(id);
    }
}

bool ResourceManager::LoadPngFile(const std::string& filePath, PngData& outData)
{
    FILE* fp = nullptr;
    fopen_s(&fp, filePath.c_str(), "rb");
    if (!fp) return false;

    png_structp png = png_create_read_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    if (!png)
    {
        fclose(fp);
        return false;
    }

    png_infop info = png_create_info_struct(png);
    if (!info)
    {
        png_destroy_read_struct(&png, nullptr, nullptr);
        fclose(fp);
        return false;
    }

    if (setjmp(png_jmpbuf(png)))
    {
        png_destroy_read_struct(&png, &info, nullptr);
        fclose(fp);
        return false;
    }

    png_init_io(png, fp);
    png_read_info(png, info);

    outData.width = png_get_image_width(png, info);
    outData.height = png_get_image_height(png, info);
    png_byte color_type = png_get_color_type(png, info);
    png_byte bit_depth = png_get_bit_depth(png, info);

    if (bit_depth == 16)
        png_set_strip_16(png);

    if (color_type == PNG_COLOR_TYPE_PALETTE)
        png_set_palette_to_rgb(png);

    if (color_type == PNG_COLOR_TYPE_GRAY && bit_depth < 8)
        png_set_expand_gray_1_2_4_to_8(png);

    if (png_get_valid(png, info, PNG_INFO_tRNS))
        png_set_tRNS_to_alpha(png);

    if (color_type == PNG_COLOR_TYPE_RGB ||
        color_type == PNG_COLOR_TYPE_GRAY ||
        color_type == PNG_COLOR_TYPE_PALETTE)
        png_set_filler(png, 0xFF, PNG_FILLER_AFTER);

    if (color_type == PNG_COLOR_TYPE_GRAY ||
        color_type == PNG_COLOR_TYPE_GRAY_ALPHA)
        png_set_gray_to_rgb(png);

    png_read_update_info(png, info);

    outData.pixels.resize(outData.width * outData.height * 4);
    std::vector<png_bytep> row_pointers(outData.height);

    for (uint32_t y = 0; y < outData.height; ++y)
        row_pointers[y] = outData.pixels.data() + y * outData.width * 4;

    png_read_image(png, row_pointers.data());
    png_destroy_read_struct(&png, &info, nullptr);
    fclose(fp);

    uint8_t* pixels = outData.pixels.data();
    const size_t pixelCount = outData.width * outData.height;

    for (size_t i = 0; i < pixelCount; ++i)
    {
        uint8_t* p = &pixels[i * 4];
        uint8_t a = p[3];
        if (a == 0)
        {
            p[0] = p[1] = p[2] = 0;
        }
        else if (a < 255)
        {
            p[0] = static_cast<uint8_t>((p[0] * a + 127) / 255);
            p[1] = static_cast<uint8_t>((p[1] * a + 127) / 255);
            p[2] = static_cast<uint8_t>((p[2] * a + 127) / 255);
        }
    }

    return true;
}

bool ResourceManager::CreateD2DBitmap(const PngData& data, ID2D1Bitmap** outBitmap)
{
    Renderer::D2DGuard guard;

    const auto d2dContext = Renderer::GetD2DContext();
    if (!d2dContext) return false;

    const D2D1_BITMAP_PROPERTIES props = D2D1::BitmapProperties(
        D2D1::PixelFormat(DXGI_FORMAT_R8G8B8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED)
    );

    return SUCCEEDED(d2dContext->CreateBitmap(
        D2D1::SizeU(data.width, data.height),
        data.pixels.data(),
        data.width * 4,
        props,
        outBitmap
    ));
}

bool ResourceManager::LoadManifest(const std::string& manifestPath)
{
    pugi::xml_document doc;
    if (!doc.load_file(manifestPath.c_str())) return false;

    resourceBasePath_ = std::filesystem::path(manifestPath).parent_path().string();

    auto root = doc.child("ResourceManifest");
    if (!root) return false;

    for (auto groupNode : root.children("Resources"))
    {
        std::string groupId = groupNode.attribute("id").as_string();
        auto& group = groups_[groupId];

        currentDefaults = DefaultSettings();

        for (auto resNode : groupNode.children())
        {
            std::string nodeName = resNode.name();

            if (nodeName == "SetDefaults")
            {
                currentDefaults.basePath = std::filesystem::path(resNode.attribute("path").as_string()).string();
                currentDefaults.idPrefix = resNode.attribute("idprefix").as_string();
            }
            else if (nodeName == "Sound")
            {
                std::string id = resNode.attribute("id").as_string();
                std::string fullId = currentDefaults.idPrefix + id;

                ResourceEntry entry;
                entry.path = (std::filesystem::path(currentDefaults.basePath) /
                    (std::string(resNode.attribute("path").as_string()) + ".ogg")).string();

                group.sounds[fullId] = entry;
            }
            else if (nodeName == "Image")
            {
                std::string id = resNode.attribute("id").as_string();
                std::string fullId = currentDefaults.idPrefix + id;

                ResourceEntry entry;
                entry.path = (std::filesystem::path(currentDefaults.basePath) /
                    std::string(resNode.attribute("path").as_string())).string();

                group.images[fullId] = entry;
            }
            else if (nodeName == "Font")
            {
                std::string id = resNode.attribute("id").as_string();
                std::string fullId = currentDefaults.idPrefix + id;

                FontEntry entry;
                entry.path = (std::filesystem::path(currentDefaults.basePath) /
                    std::string(resNode.attribute("path").as_string())).string();
                const std::string family = resNode.attribute("family").as_string();
                entry.fontFamily = std::wstring(family.begin(), family.end());

                group.fonts[fullId] = entry;
            }
        }
    }

    return true;
}

bool ResourceManager::LoadGroup(const std::string& groupName)
{
    {
        std::lock_guard lock(groupsMutex_);
        const auto itFind = groups_.find(groupName);
        if (itFind == groups_.end()) return false;
        if (itFind->second.isLoaded) return true;
    }

    {
        std::lock_guard lock(groupsMutex_);
        auto& group = groups_[groupName];
        for (auto& [id, entry] : group.sounds)
        {
            if (!entry.loaded)
            {
                std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string();
                if (AudioManager::PreloadAudio(id, fullPath))
                {
                    entry.loaded = true;
                }
                else
                {
                    std::cout << "Error while create audio: " << id << "\n";
                }
            }
        }
    }

    {
        std::lock_guard lock(groupsMutex_);
        auto& group = groups_[groupName];
        for (auto& [id, entry] : group.images)
        {
            if (!entry.loaded)
            {
                std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string() + ".png";
                if (PngData pngData; LoadPngFile(fullPath, pngData))
                {
                    ID2D1Bitmap* bitmap = nullptr;
                    if (CreateD2DBitmap(pngData, &bitmap))
                    {
                        images_[id].Attach(bitmap);
                        entry.loaded = true;
                    }
                    else
                    {
                        std::cout << "Error while create image: " << id << "\n";
                    }
                }
                else
                {
                    std::cout << "Error while load image: " << id << "\n";
                }
            }
        }
    }

    {
        std::lock_guard lock(groupsMutex_);
        auto& group = groups_[groupName];
        for (auto& [id, entry] : group.fonts)
        {
            if (!entry.loaded)
            {
                std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string() + ".ttf";
                if (LoadFont(id, fullPath, entry.fontFamily))
                {
                    entry.loaded = true;
                }
                else
                {
                    std::cout << "Error while load font: " << id << "\n";
                }
            }
        }

        group.isLoaded = true;
    }

    return true;
}

ID2D1Bitmap* ResourceManager::GetImage(const std::string& id)
{
    std::lock_guard lock(groupsMutex_);

    for (auto& group : groups_ | std::views::values)
    {
        if (auto it = group.images.find(id); it != group.images.end())
        {
            if (!it->second.loaded)
            {
                const std::string fullPath = (std::filesystem::path(resourceBasePath_) / it->second.path).string();
                PngData pngData;
                if (LoadPngFile(fullPath, pngData))
                {
                    ID2D1Bitmap* bitmap = nullptr;
                    if (CreateD2DBitmap(pngData, &bitmap))
                    {
                        images_[id].Attach(bitmap);
                        it->second.loaded = true;
                    }
                }
            }
            const auto found = images_.find(id);
            return found != images_.end() ? found->second.Get() : nullptr;
        }
    }
    return nullptr;
}

std::string ResourceManager::GetAudio(const std::string& id)
{
    for (auto& group : groups_ | std::views::values)
    {
        if (auto it = group.sounds.find(id); it != group.sounds.end())
        {
            if (!it->second.loaded)
            {
                const std::string fullPath = (std::filesystem::path(resourceBasePath_) / it->second.path).string();
                if (AudioManager::PreloadAudio(id, fullPath))
                {
                    it->second.loaded = true;
                }
            }
            return (std::filesystem::path(resourceBasePath_) / it->second.path).string();
        }
    }
    return "";
}

std::string ResourceManager::TokenToReanimFileName(const std::string& id)
{
    std::string token = id;
    const std::string prefix = "IMAGE_REANIM_";
    if (token.starts_with(prefix))
        token = token.substr(prefix.size());

    std::vector<std::string> parts;
    std::string cur;
    for (char c : token)
    {
        if (c == '_')
        {
            if (!cur.empty())
            {
                parts.push_back(cur);
                cur.clear();
            }
        }
        else
        {
            cur.push_back(c);
        }
    }
    if (!cur.empty()) parts.push_back(cur);

    std::string result;
    for (size_t i = 0; i < parts.size(); ++i)
    {
        std::string p = parts[i];
        for (auto& ch : p) ch = static_cast<char>(tolower(static_cast<unsigned char>(ch)));
        if (i == 0 && !p.empty()) p[0] = static_cast<char>(toupper(static_cast<unsigned char>(p[0])));
        if (!result.empty()) result.push_back('_');
        result += p;
    }
    return result;
}
