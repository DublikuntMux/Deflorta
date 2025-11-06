#include "ResourceManager.hpp"

#include "AudioManager.hpp"

#include <pugixml.hpp>
#include <png.h>
#include <Windows.h>

#include <algorithm>
#include <filesystem>
#include <iostream>
#include <ranges>
#include <vector>

IRenderBackend* ResourceManager::backend_ = nullptr;
std::unordered_map<std::string, ResourceGroup> ResourceManager::groups_;
std::unordered_map<std::string, std::shared_ptr<ITexture>> ResourceManager::images_;
std::unordered_map<std::string, std::wstring> ResourceManager::fonts_;
std::string ResourceManager::resourceBasePath_;
DefaultSettings ResourceManager::currentDefaults;
std::mutex ResourceManager::groupsMutex_;

void ResourceManager::SetRenderBackend(IRenderBackend* backend)
{
    backend_ = backend;
}

bool ResourceManager::LoadFont(const std::string& id, const std::string& filePath, const std::wstring& familyName)
{
    const std::wstring wpath = std::filesystem::path(filePath).wstring();
    const int added = AddFontResourceExW(wpath.c_str(), FR_PRIVATE, nullptr);
    if (added <= 0)
    {
        std::cout << "Error: Failed to load font '" << id << "' from path: " << filePath << "\n";
        return false;
    }

    fonts_[id] = familyName;
    return true;
}

std::wstring ResourceManager::GetFont(const std::string& id)
{
    std::lock_guard lock(groupsMutex_);
    const auto it = fonts_.find(id);
    if (it == fonts_.end())
    {
        std::cout << "Warning: Font '" << id << "' not found, using ID as font family name\n";
        return std::wstring(id.begin(), id.end());
    }
    return it->second;
}

bool ResourceManager::PreloadReanimImage(const std::string& id)
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
                    PixelData pixelData;
                    if (LoadPngFile(fullPath, pixelData))
                    {
                        std::shared_ptr<ITexture> texture;
                        if (CreateTexture(pixelData, &texture))
                        {
                            images_[id] = texture;
                            it->second.loaded = true;
                        }
                        else
                        {
                            std::cout << "Error: Failed to create texture for reanim image '" << id << "'\n";
                        }
                    }
                    else
                    {
                        std::cout << "Error: Failed to load PNG file '" << fullPath << "' for reanim image '" << id <<
                            "'\n";
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
            entry.path = (std::filesystem::path("reanim") / fileNameNoExt).string();

            const std::string fullPath = (std::filesystem::path(resourceBasePath_) / entry.path).string() + ".png";
            PixelData pixelData;
            if (LoadPngFile(fullPath, pixelData))
            {
                std::shared_ptr<ITexture> texture;
                if (CreateTexture(pixelData, &texture))
                {
                    images_[id] = texture;
                    entry.loaded = true;
                }
                else
                {
                    std::cout << "Error: Failed to create texture for reanim image '" << id << "'\n";
                }
            }
            else
            {
                std::cout << "Error: Failed to load PNG file '" << fullPath << "' for reanim image '" << id << "'\n";
            }
        }
        return images_.contains(id);
    }
}

bool ResourceManager::LoadPngFile(const std::string& filePath, PixelData& outData)
{
    FILE* fp = nullptr;
    fopen_s(&fp, filePath.c_str(), "rb");
    if (!fp)
    {
        std::cout << "Error: Failed to open PNG file: " << filePath << "\n";
        return false;
    }

    png_structp png = png_create_read_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    if (!png)
    {
        std::cout << "Error: Failed to create PNG read struct for file: " << filePath << "\n";
        fclose(fp);
        return false;
    }

    png_infop info = png_create_info_struct(png);
    if (!info)
    {
        std::cout << "Error: Failed to create PNG info struct for file: " << filePath << "\n";
        png_destroy_read_struct(&png, nullptr, nullptr);
        fclose(fp);
        return false;
    }

    if (setjmp(png_jmpbuf(png)))
    {
        std::cout << "Error: PNG reading failed for file: " << filePath << "\n";
        png_destroy_read_struct(&png, &info, nullptr);
        fclose(fp);
        return false;
    }

    png_init_io(png, fp);
    png_read_info(png, info);

    outData.width = png_get_image_width(png, info);
    outData.height = png_get_image_height(png, info);
    const png_byte color_type = png_get_color_type(png, info);
    const png_byte bit_depth = png_get_bit_depth(png, info);

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
        const uint8_t a = p[3];
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

bool ResourceManager::CreateTexture(const PixelData& data, std::shared_ptr<ITexture>* outTexture)
{
    if (!backend_)
    {
        std::cout << "Error: Render backend not set in ResourceManager\n";
        return false;
    }

    if (!outTexture) return false;

    RenderBackendLock lock(backend_);

    *outTexture = backend_->CreateTexture(data);
    return *outTexture != nullptr;
}

bool ResourceManager::LoadManifest(const std::string& manifestPath)
{
    pugi::xml_document doc;
    if (!doc.load_file(manifestPath.c_str()))
    {
        std::cout << "Error: Failed to load manifest file: " << manifestPath << "\n";
        return false;
    }

    resourceBasePath_ = std::filesystem::path(manifestPath).parent_path().string();

    auto root = doc.child("ResourceManifest");
    if (!root)
    {
        std::cout << "Error: Invalid manifest format - missing 'ResourceManifest' root element in: " << manifestPath <<
            "\n";
        return false;
    }

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

                if (auto rowsAttr = resNode.attribute("rows"))
                    entry.rows = rowsAttr.as_int(1);
                if (auto colsAttr = resNode.attribute("cols"))
                    entry.cols = colsAttr.as_int(1);

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
        if (itFind == groups_.end())
        {
            std::cout << "Error: Resource group '" << groupName << "' not found in manifest\n";
            return false;
        }
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
                    std::cout << "Error: Failed to load audio '" << id << "' from path: " << fullPath << "\n";
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
                if (PixelData pixelData; LoadPngFile(fullPath, pixelData))
                {
                    if (entry.rows > 1 || entry.cols > 1)
                    {
                        if (CreateSlicedImages(id, pixelData, entry.rows, entry.cols))
                        {
                            entry.loaded = true;
                        }
                        else
                        {
                            std::cout << "Error: Failed to slice image '" << id << "'\n";
                        }
                    }
                    else
                    {
                        std::shared_ptr<ITexture> texture;
                        if (CreateTexture(pixelData, &texture))
                        {
                            images_[id] = texture;
                            entry.loaded = true;
                        }
                        else
                        {
                            std::cout << "Error: Failed to create texture for image '" << id << "'\n";
                        }
                    }
                }
                else
                {
                    std::cout << "Error: Failed to load PNG file for image '" << id << "' from path: " << fullPath <<
                        "\n";
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
                    std::cout << "Error: Failed to load font '" << id << "' from path: " << fullPath << "\n";
                }
            }
        }

        group.isLoaded = true;
    }
    return true;
}

std::shared_ptr<ITexture> ResourceManager::GetImage(const std::string& id)
{
    std::lock_guard lock(groupsMutex_);

    if (const auto found = images_.find(id); found != images_.end())
    {
        return found->second;
    }

    std::string baseId = id;
    const size_t lastUnderscore = id.find_last_of('_');
    if (lastUnderscore != std::string::npos)
    {
        std::string suffix = id.substr(lastUnderscore + 1);
        if (!suffix.empty() && std::ranges::all_of(suffix, isdigit))
        {
            baseId = id.substr(0, lastUnderscore);
        }
    }

    for (auto& group : groups_ | std::views::values)
    {
        if (auto it = group.images.find(baseId); it != group.images.end())
        {
            if (!it->second.loaded)
            {
                const std::string fullPath = (std::filesystem::path(resourceBasePath_) / it->second.path).string() +
                    ".png";
                PixelData pixelData;
                if (LoadPngFile(fullPath, pixelData))
                {
                    if (it->second.rows > 1 || it->second.cols > 1)
                    {
                        if (CreateSlicedImages(baseId, pixelData, it->second.rows, it->second.cols))
                        {
                            it->second.loaded = true;
                        }
                        else
                        {
                            std::cout << "Error: Failed to slice image '" << baseId << "'\n";
                        }
                    }
                    else
                    {
                        std::shared_ptr<ITexture> texture;
                        if (CreateTexture(pixelData, &texture))
                        {
                            images_[baseId] = texture;
                            it->second.loaded = true;
                        }
                        else
                        {
                            std::cout << "Error: Failed to create texture for image '" << baseId << "'\n";
                        }
                    }
                }
                else
                {
                    std::cout << "Error: Failed to load PNG file for image '" << baseId << "' from path: " << fullPath
                        <<
                        "\n";
                }
            }

            const auto found = images_.find(id);
            if (found == images_.end())
            {
                std::cout << "Error: Image '" << id << "' not found in loaded images after load attempt\n";
                return nullptr;
            }
            return found->second;
        }
    }
    std::cout << "Error: Image '" << id << "' not found in any resource group\n";
    return nullptr;
}

void ResourceManager::PreloadAudio(const std::string& id)
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
                else
                {
                    std::cout << "Error: Failed to preload audio '" << id << "' from path: " << fullPath << "\n";
                }
            }
            return;
        }
    }
    std::cout << "Error: Audio resource '" << id << "' not found in any resource group\n";
}

std::string ResourceManager::TokenToReanimFileName(const std::string& id)
{
    std::string token = id;
    const std::string prefix = "IMAGE_REANIM_";
    if (token.starts_with(prefix))
        token = token.substr(prefix.size());

    std::vector<std::string> parts;
    std::string cur;
    for (const char c : token)
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

bool ResourceManager::CreateSlicedImages(const std::string& baseId, const PixelData& sourceData, int rows, int cols)
{
    if (rows <= 0 || cols <= 0)
    {
        std::cout << "Error: Invalid rows/cols for slicing image '" << baseId << "'\n";
        return false;
    }

    const uint32_t tileWidth = sourceData.width / cols;
    const uint32_t tileHeight = sourceData.height / rows;

    if (tileWidth == 0 || tileHeight == 0)
    {
        std::cout << "Error: Tile dimensions too small for image '" << baseId << "'\n";
        return false;
    }

    int index = 0;
    for (int row = 0; row < rows; ++row)
    {
        for (int col = 0; col < cols; ++col)
        {
            PixelData tileData;
            tileData.width = tileWidth;
            tileData.height = tileHeight;
            tileData.pixels.resize(tileWidth * tileHeight * 4);

            for (uint32_t y = 0; y < tileHeight; ++y)
            {
                const uint32_t srcY = row * tileHeight + y;
                const uint32_t srcOffset = (srcY * sourceData.width + col * tileWidth) * 4;
                const uint32_t dstOffset = y * tileWidth * 4;

                std::memcpy(
                    tileData.pixels.data() + dstOffset,
                    sourceData.pixels.data() + srcOffset,
                    tileWidth * 4
                );
            }

            std::shared_ptr<ITexture> texture;
            if (CreateTexture(tileData, &texture))
            {
                const std::string slicedId = baseId + "_" + std::to_string(index);
                images_[slicedId] = texture;
            }
            else
            {
                std::cout << "Error: Failed to create texture for sliced image '" << baseId << "_" << index << "'\n";
                return false;
            }

            ++index;
        }
    }

    return true;
}

bool ResourceManager::LoadReanimImageData(const std::string& id, PixelData& outData)
{
    {
        std::lock_guard lock(groupsMutex_);
        for (auto& group : groups_ | std::views::values)
        {
            if (auto it = group.images.find(id); it != group.images.end())
            {
                const std::string fullPath = (std::filesystem::path(resourceBasePath_) / it->second.path).string() +
                    ".png";
                return LoadPngFile(fullPath, outData);
            }
        }
    }

    const std::string fileNameNoExt = TokenToReanimFileName(id);
    const std::string reanimPath = (std::filesystem::path("reanim") / fileNameNoExt).string();
    const std::string fullPath = (std::filesystem::path(resourceBasePath_) / reanimPath).string() + ".png";

    return LoadPngFile(fullPath, outData);
}

std::shared_ptr<ITexture> ResourceManager::CreateTextureFromPixelData(const PixelData& data)
{
    std::shared_ptr<ITexture> texture;
    if (CreateTexture(data, &texture))
    {
        return texture;
    }
    return nullptr;
}
