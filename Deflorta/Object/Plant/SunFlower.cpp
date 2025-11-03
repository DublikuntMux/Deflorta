#include "SunFlower.hpp"

#include <stdexcept>

#include "../../Render/Layer.hpp"

SunFlower::SunFlower() : BasePlant(PlantType::SunFlower, PlantPos::Base)
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/SunFlower.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load SunFlower reanim");

    sprite = std::make_unique<Reanimator>(reanim.value());
    sprite->PlayLayer("anim_idle", ReanimLoopType::Loop);
    sprite->SetAllLayersZ(static_cast<int>(RenderLayer::PlantBase));
}
