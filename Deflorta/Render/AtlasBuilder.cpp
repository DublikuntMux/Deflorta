#include "AtlasBuilder.hpp"

#include <png.h>

#include <algorithm>
#include <iostream>
#include <ranges>

#include "../Base/Random.hpp"

bool TextureAtlas::Save(const std::string& filePath) const
{
    if (atlasData.width == 0 || atlasData.height == 0 || atlasData.pixels.empty())
    {
        std::cerr << "TextureAtlas::SaveToPNG: No image data to save\n";
        return false;
    }

    FILE* fp = std::fopen(filePath.c_str(), "wb");
    if (!fp)
    {
        std::cerr << "TextureAtlas::SaveToPNG: Failed to open file '" << filePath << "'\n";
        return false;
    }

    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    if (!png_ptr)
    {
        std::cerr << "TextureAtlas::SaveToPNG: Failed to create write struct\n";
        std::fclose(fp);
        return false;
    }

    png_infop info_ptr = png_create_info_struct(png_ptr);
    if (!info_ptr)
    {
        std::cerr << "TextureAtlas::SaveToPNG: Failed to create info struct\n";
        png_destroy_write_struct(&png_ptr, nullptr);
        std::fclose(fp);
        return false;
    }

    if (setjmp(png_jmpbuf(png_ptr)))
    {
        std::cerr << "TextureAtlas::SaveToPNG: libpng encountered an error\n";
        png_destroy_write_struct(&png_ptr, &info_ptr);
        std::fclose(fp);
        return false;
    }

    png_init_io(png_ptr, fp);

    png_set_IHDR(
        png_ptr, info_ptr,
        atlasData.width,
        atlasData.height,
        8,
        PNG_COLOR_TYPE_RGBA,
        PNG_INTERLACE_NONE,
        PNG_COMPRESSION_TYPE_BASE,
        PNG_FILTER_TYPE_BASE
    );

    png_write_info(png_ptr, info_ptr);

    std::vector<png_bytep> row_pointers(atlasData.height);
    for (uint32_t y = 0; y < atlasData.height; ++y)
    {
        row_pointers[y] = const_cast<png_bytep>(
            &atlasData.pixels[y * atlasData.width * 4]
        );
    }

    png_write_image(png_ptr, row_pointers.data());
    png_write_end(png_ptr, nullptr);

    png_destroy_write_struct(&png_ptr, &info_ptr);
    std::fclose(fp);
    return true;
}

void AtlasBuilder::AddImage(const std::string& id, const PixelData& pixelData)
{
    if (pixelData.width == 0 || pixelData.height == 0 || pixelData.pixels.empty())
    {
        std::cerr << "AtlasBuilder: Skipping empty image '" << id << "'\n";
        return;
    }

    images_.push_back({.id = id, .data = pixelData});
}

void AtlasBuilder::Clear()
{
    images_.clear();
}

AtlasBuilder::PackNode* AtlasBuilder::FindNode(PackNode* root, uint32_t width, uint32_t height)
{
    if (!root) return nullptr;

    if (root->used)
    {
        if (PackNode* newNode = FindNode(root->right, width, height)) return newNode;
        return FindNode(root->down, width, height);
    }

    if (width <= root->width && height <= root->height)
    {
        return root;
    }

    return nullptr;
}

AtlasBuilder::PackNode* AtlasBuilder::SplitNode(PackNode* node, uint32_t width, uint32_t height)
{
    if (!node) return nullptr;

    node->used = true;
    node->down = new PackNode();
    node->down->x = node->x;
    node->down->y = node->y + height;
    node->down->width = node->width;
    node->down->height = node->height - height;

    node->right = new PackNode();
    node->right->x = node->x + width;
    node->right->y = node->y;
    node->right->width = node->width - width;
    node->right->height = height;

    return node;
}

void AtlasBuilder::CopyImageToAtlas(const PixelData& src, PixelData& dst, uint32_t x, uint32_t y)
{
    for (uint32_t row = 0; row < src.height; ++row)
    {
        const uint32_t srcOffset = row * src.width * 4;
        const uint32_t dstOffset = ((y + row) * dst.width + x) * 4;

        std::memcpy(
            dst.pixels.data() + dstOffset,
            src.pixels.data() + srcOffset,
            src.width * 4
        );
    }
}

bool AtlasBuilder::Build(TextureAtlas& outAtlas, uint32_t maxAtlasSize, uint32_t padding)
{
    if (images_.empty())
    {
        std::cerr << "AtlasBuilder: No images to pack\n";
        return false;
    }

    std::ranges::sort(images_,
                      [](const ImageEntry& a, const ImageEntry& b)
                      {
                          return a.data.height > b.data.height;
                      });

    uint32_t totalArea = 0;
    uint32_t maxWidth = 0;
    uint32_t maxHeight = 0;

    for (const auto& [id, data] : images_)
    {
        uint32_t w = data.width + padding * 2;
        uint32_t h = data.height + padding * 2;
        totalArea += w * h;
        maxWidth = std::max(maxWidth, w);
        maxHeight = std::max(maxHeight, h);
    }

    uint32_t atlasSize = 256;
    while (atlasSize < maxWidth || atlasSize < maxHeight || atlasSize * atlasSize < totalArea)
    {
        atlasSize *= 2;
        if (atlasSize > maxAtlasSize)
        {
            atlasSize = maxAtlasSize;
            break;
        }
    }

    PackNode* root = nullptr;
    bool success = false;

    while (atlasSize <= maxAtlasSize && !success)
    {
        delete root;
        root = new PackNode();
        root->x = 0;
        root->y = 0;
        root->width = atlasSize;
        root->height = atlasSize;

        success = true;
        outAtlas.regions.clear();

        for (const auto& [id, data] : images_)
        {
            const uint32_t w = data.width + padding * 2;
            const uint32_t h = data.height + padding * 2;

            if (PackNode* node = FindNode(root, w, h))
            {
                if (const PackNode* fit = SplitNode(node, w, h))
                {
                    AtlasRegion region;
                    region.x = fit->x + padding;
                    region.y = fit->y + padding;
                    region.width = data.width;
                    region.height = data.height;
                    region.pixelSize = glm::vec2(data.width, data.height);

                    outAtlas.regions[id] = region;
                }
                else
                {
                    success = false;
                    break;
                }
            }
            else
            {
                success = false;
                break;
            }
        }

        if (!success)
        {
            atlasSize *= 2;
        }
    }

    if (!success)
    {
        delete root;
        std::cerr << "AtlasBuilder: Failed to pack images into atlas (max size: "
            << maxAtlasSize << "x" << maxAtlasSize << ")\n";
        return false;
    }

    outAtlas.atlasWidth = atlasSize;
    outAtlas.atlasHeight = atlasSize;
    outAtlas.atlasData.width = atlasSize;
    outAtlas.atlasData.height = atlasSize;
    outAtlas.atlasData.pixels.resize(atlasSize * atlasSize * 4, 0);

    for (const auto& [id, data] : images_)
    {
        const auto& region = outAtlas.regions[id];
        CopyImageToAtlas(data, outAtlas.atlasData, region.x, region.y);
    }

    for (auto& region : outAtlas.regions | std::views::values)
    {
        region.uvMin.x = static_cast<float>(region.x) / static_cast<float>(atlasSize);
        region.uvMin.y = static_cast<float>(region.y) / static_cast<float>(atlasSize);
        region.uvMax.x = static_cast<float>(region.x + region.width) / static_cast<float>(atlasSize);
        region.uvMax.y = static_cast<float>(region.y + region.height) / static_cast<float>(atlasSize);
    }

    delete root;

    return true;
}
