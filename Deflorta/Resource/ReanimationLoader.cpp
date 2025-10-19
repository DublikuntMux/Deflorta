#include "ReanimationLoader.hpp"

#include <iostream>

#include "ResourceManager.hpp"

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

    read_float("x", t.transX);
    read_float("y", t.transY);
    read_float("kx", t.skewX);
    read_float("ky", t.skewY);
    read_float("sx", t.scaleX);
    read_float("sy", t.scaleY);
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

    float prevX = 0, prevY = 0;
    float prevKx = 0, prevKy = 0;
    float prevSx = 1, prevSy = 1;
    float prevF = 0, prevA = 1;
    std::string prevImg, prevFont, prevText;

    for (auto& [transX, transY, skewX, skewY, scaleX, scaleY, frame, alpha, image, font, text] : track.transforms)
    {
        auto fill = [&](float& field, float prev, float defVal)
        {
            if (field == defVal) field = prev;
        };

        fill(transX, prevX, REANIM_MISSING);
        fill(transY, prevY, REANIM_MISSING);
        fill(skewX, prevKx, REANIM_MISSING);
        fill(skewY, prevKy, REANIM_MISSING);
        fill(scaleX, prevSx, REANIM_MISSING);
        fill(scaleY, prevSy, REANIM_MISSING);
        fill(frame, prevF, REANIM_MISSING);
        fill(alpha, prevA, REANIM_MISSING);

        if (image.empty()) image = prevImg;
        if (font.empty()) font = prevFont;
        if (text.empty()) text = prevText;

        prevX = transX;
        prevY = transY;
        prevKx = skewX;
        prevKy = skewY;
        prevSx = scaleX;
        prevSy = scaleY;
        prevF = frame;
        prevA = alpha;
        prevImg = image;
        prevFont = font;
        prevText = text;
    }
}
