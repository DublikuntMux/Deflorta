#include "SunFlower.hpp"

#include <stdexcept>

SunFlower::SunFlower() : BasePlant(PlantType::SunFlower, PlantPos::Base)
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/SunFlower.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load SunFlower reanim");

    sprite = std::make_unique<Reanimator>(reanim.value());
    sprite->PlayLayer("anim_idle", ReanimLoopType::Loop);
}
