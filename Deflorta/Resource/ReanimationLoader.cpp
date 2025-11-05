#include "ReanimationLoader.hpp"

#include "ResourceManager.hpp"

#include <iostream>

std::unordered_map<std::string, ReanimatorDefinition> ReanimationLoader::loadedReanimations_;

std::optional<ReanimatorDefinition*> ReanimationLoader::LoadFromFile(const std::string& path)
{
    if (path.empty())
        return std::nullopt;

    if (const auto it = loadedReanimations_.find(path); it != loadedReanimations_.end())
        return &it->second;

    pugi::xml_document doc;
    const pugi::xml_parse_result result = doc.load_file(path.c_str());
    if (!result)
    {
        std::cerr << "ReanimationLoader: Failed to load XML " << path
            << " (" << result.description() << ")\n";
        return std::nullopt;
    }

    ReanimatorDefinition def;
    const pugi::xml_node root = doc.child("reanim");
    if (!root)
    {
        std::cerr << "ReanimationLoader: Missing <reanim> root\n";
        return std::nullopt;
    }

    if (const auto fpsNode = root.child("fps"))
        def.fps = fpsNode.text().as_float(12.0f);

    for (auto trackNode : root.children("track"))
    {
        ReanimatorTrack track;
        if (auto nameNode = trackNode.child("name"))
            track.name = nameNode.text().as_string();

        for (auto tNode : trackNode.children("t"))
            track.transforms.push_back(ParseTransform(tNode));

        FillMissingData(track);
        def.tracks.push_back(std::move(track));
    }

    for (auto& [name, transforms] : def.tracks)
    {
        for (auto& tr : transforms)
        {
            if (!tr.image.empty())
            {
                ResourceManager::PreloadReanimImage(tr.image);
            }
        }
    }

    auto [it, inserted] = loadedReanimations_.emplace(path, std::move(def));
    return &it->second;
}

ReanimatorTransform ReanimationLoader::ParseTransform(const pugi::xml_node& node)
{
    ReanimatorTransform t;

    auto read_float = [&](const char* tag, float& out)
    {
        if (const auto n = node.child(tag))
            out = n.text().as_float();
    };
    auto read_string = [&](const char* tag, std::string& out)
    {
        if (const auto n = node.child(tag))
            out = n.text().as_string();
    };

    read_float("x", t.translation.x);
    read_float("y", t.translation.y);
    read_float("kx", t.skew.x);
    read_float("ky", t.skew.y);
    read_float("sx", t.scale.x);
    read_float("sy", t.scale.y);
    read_float("f", t.frame);
    read_float("a", t.alpha);
    read_string("i", t.image);
    read_string("font", t.font);
    read_string("text", t.text);

    return t;
}

void ReanimationLoader::FillMissingData(ReanimatorTrack& track)
{
    if (track.transforms.empty()) return;

    glm::vec2 prevTranslation = {0.0f, 0.0f};
    glm::vec2 prevSkew = {0.0f, 0.0f};
    glm::vec2 prevScale = {1.0f, 1.0f};
    float prevF = 0, prevA = 1;
    std::string prevImg, prevFont, prevText;

    for (auto& t : track.transforms)
    {
        auto fill = [](float& field, float prev, float defVal)
        {
            if (field == defVal) field = prev;
        };

        fill(t.translation.x, prevTranslation.x, REANIM_MISSING);
        fill(t.translation.y, prevTranslation.y, REANIM_MISSING);
        fill(t.skew.x, prevSkew.x, REANIM_MISSING);
        fill(t.skew.y, prevSkew.y, REANIM_MISSING);
        fill(t.scale.x, prevScale.x, REANIM_MISSING);
        fill(t.scale.y, prevScale.y, REANIM_MISSING);
        fill(t.frame, prevF, REANIM_MISSING);
        fill(t.alpha, prevA, REANIM_MISSING);

        if (t.image.empty()) t.image = prevImg;
        if (t.font.empty()) t.font = prevFont;
        if (t.text.empty()) t.text = prevText;

        prevTranslation = t.translation;
        prevSkew = t.skew;
        prevScale = t.scale;
        prevF = t.frame;
        prevA = t.alpha;
        prevImg = t.image;
        prevFont = t.font;
        prevText = t.text;
    }
}
