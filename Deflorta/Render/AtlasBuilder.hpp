#pragma once

#include "PixelData.hpp"

#include <glm/vec2.hpp>

#include <string>
#include <unordered_map>
#include <vector>

struct AtlasRegion
{
    glm::vec2 uvMin;
    glm::vec2 uvMax;
    glm::vec2 pixelSize;
    uint32_t x, y;
    uint32_t width, height;
};

struct TextureAtlas
{
    PixelData atlasData;
    std::unordered_map<std::string, AtlasRegion> regions;
    uint32_t atlasWidth = 0;
    uint32_t atlasHeight = 0;

    bool Save(const std::string& filePath) const;
};

class AtlasBuilder
{
public:
    void AddImage(const std::string& id, const PixelData& pixelData);
    bool Build(TextureAtlas& outAtlas, uint32_t maxAtlasSize = 4096, uint32_t padding = 1);
    void Clear();

private:
    struct ImageEntry
    {
        std::string id;
        PixelData data;
    };

    std::vector<ImageEntry> images_;

    struct PackNode
    {
        uint32_t x = 0, y = 0;
        uint32_t width = 0, height = 0;
        bool used = false;
        PackNode* right = nullptr;
        PackNode* down = nullptr;

        ~PackNode()
        {
            delete right;
            delete down;
        }
    };

    static PackNode* FindNode(PackNode* root, uint32_t width, uint32_t height);
    static PackNode* SplitNode(PackNode* node, uint32_t width, uint32_t height);
    static void CopyImageToAtlas(const PixelData& src, PixelData& dst, uint32_t x, uint32_t y);
};
