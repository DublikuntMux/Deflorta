#pragma once

#include <string>
#include <vector>
#include <optional>

#include <pugixml.hpp>
#include <unordered_map>

constexpr float REANIM_MISSING = -10000.0f;

struct ReanimatorTransform
{
    float transX = REANIM_MISSING;
    float transY = REANIM_MISSING;
    float skewX = REANIM_MISSING;
    float skewY = REANIM_MISSING;
    float scaleX = REANIM_MISSING;
    float scaleY = REANIM_MISSING;
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
