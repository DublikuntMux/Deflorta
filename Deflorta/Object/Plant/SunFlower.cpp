#include "SunFlower.hpp"

#include "../../Render/Layer.hpp"
#include "../../Base/Random.hpp"
#include "../../Scene/Scene.hpp"
#include "../Clickable/SunObject.hpp"
#include "../Clickable/SpawnAnimation.hpp"

#include <stdexcept>

SunFlower::SunFlower() : BasePlant(PlantType::SunFlower, PlantLayer::Base)
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/SunFlower.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load SunFlower reanim");

    sprite = std::make_unique<Reanimator>(reanim.value());
    sprite->PlayLayer("anim_idle", ReanimLoopType::Loop);
    sprite->SetAllLayersZ(static_cast<int>(RenderLayer::PlantBase));

    timer_ = std::make_unique<Timer>();
    timer_->SetTimeoutCallback([this] { ProduceSun(); });
    timer_->Start(Random::UniformFloat(24.0f, 25.0f));
}

void SunFlower::Update()
{
    BasePlant::Update();
    timer_->Update();
}

void SunFlower::ProduceSun()
{
    timer_->SetWaitTime(Random::UniformFloat(23.0f, 25.0f));
    auto position = GetTransform().position;
    position.x += 50.0f;
    auto sun = Create<SunObject>(position, 50, SpawnAnimation::Produce);
    scene_->AddGameObject(std::move(sun));
}
