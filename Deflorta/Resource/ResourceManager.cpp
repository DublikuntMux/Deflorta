#include "ResourceManager.hpp"

#include <filesystem>
#include <ranges>
#include <vector>

#include <pugixml.hpp>
#include <png.h>

#include "../Render/Renderer.hpp"

std::unordered_map<std::string, ResourceGroup> ResourceManager::groups_;
std::string ResourceManager::resourceBasePath_;
DefaultSettings ResourceManager::currentDefaults;
std::mutex ResourceManager::groupsMutex_;

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

    std::vector<png_bytep> row_pointers(outData.height);
    outData.pixels.resize(outData.width * outData.height * 4);

    for (uint32_t y = 0; y < outData.height; y++)
    {
        row_pointers[y] = outData.pixels.data() + y * outData.width * 4;
    }

    png_read_image(png, row_pointers.data());

    png_destroy_read_struct(&png, &info, nullptr);
    fclose(fp);
    return true;
}

bool ResourceManager::CreateD2DBitmap(const PngData& data, ID2D1Bitmap** outBitmap)
{
    Renderer::D2DGuard guard;

    auto d2dContext = Renderer::GetD2DContext();
    if (!d2dContext) return false;

    D2D1_BITMAP_PROPERTIES props = D2D1::BitmapProperties(
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
            }
        }
    }

    {
        std::lock_guard lock(groupsMutex_);
        auto& group = groups_[groupName];
        for (auto& entry : group.images | std::views::values)
        {
            if (!entry.loaded)
            {
                std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string() + ".png";
                if (PngData pngData; LoadPngFile(fullPath, pngData))
                {
                    ID2D1Bitmap* bitmap = nullptr;
                    if (CreateD2DBitmap(pngData, &bitmap))
                    {
                        entry.bitmap.Attach(bitmap);
                        entry.loaded = true;
                    }
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
                        it->second.bitmap.Attach(bitmap);
                        it->second.loaded = true;
                    }
                }
            }
            return it->second.bitmap.Get();
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
