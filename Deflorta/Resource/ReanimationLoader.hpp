#pragma once

#include <glm/vec2.hpp>
#include <pugixml.hpp>

#include <string>
#include <vector>
#include <optional>
#include <unordered_map>
#include <memory>

constexpr float REANIM_MISSING = -10000.0f;

class ITexture;
struct AtlasRegion;

struct ReanimatorTransform
{
    glm::vec2 translation = {REANIM_MISSING, REANIM_MISSING};
    glm::vec2 skew = {REANIM_MISSING, REANIM_MISSING};
    glm::vec2 scale = {REANIM_MISSING, REANIM_MISSING};
    float frame = REANIM_MISSING;
    float alpha = REANIM_MISSING;

    std::string image;
    std::string font;
    std::string text;
};

struct ReanimatorTrack
{
    std::string name;
    std::vector<ReanimatorTransform> transforms;
};

struct ReanimatorDefinition
{
    std::vector<ReanimatorTrack> tracks;
    float fps = 12.0f;

    std::shared_ptr<ITexture> atlasTexture;
    std::unordered_map<std::string, AtlasRegion> atlasRegions;
    bool useAtlas = false;
};

class ReanimationLoader
{
public:
    static std::optional<ReanimatorDefinition*> LoadFromFile(const std::string& path);

private:
    static ReanimatorTransform ParseTransform(const pugi::xml_node& node);
    static void FillMissingData(ReanimatorTrack& track);

    static std::unordered_map<std::string, ReanimatorDefinition> loadedReanimations_;
};
