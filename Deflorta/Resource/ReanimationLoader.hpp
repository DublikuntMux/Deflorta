#pragma once

#include <string>
#include <vector>
#include <optional>

#include <pugixml.hpp>

struct ReanimatorTransform
{
    float transX = -10000.0f;
    float transY = -10000.0f;
    float skewX = -10000.0f;
    float skewY = -10000.0f;
    float scaleX = -10000.0f;
    float scaleY = -10000.0f;
    float frame = -10000.0f;
    float alpha = -10000.0f;

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
    static std::optional<ReanimatorDefinition> LoadFromFile(const std::string& path);

private:
    static ReanimatorTransform ParseTransform(const pugi::xml_node& node);
    static void FillMissingData(ReanimatorTrack& track);
};
