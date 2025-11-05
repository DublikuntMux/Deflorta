#include "SunObject.hpp"

#include <stdexcept>

SunObject::SunObject(): ClickableObject(GameObjectTag::Sun)
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/Sun.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load Sun reanim");

    sprite = std::make_unique<Reanimator>(reanim.value());
    sprite->PlayLayer("Sun1", ReanimLoopType::Loop);
    sprite->SetAllLayersZ(static_cast<int>(RenderLayer::Collectable));

    auto collider = std::make_unique<CircleCollider>(this,30.0f);
    SetCollider(std::move(collider));
}

void SunObject::Render()
{
    ClickableObject::Render();

    if (sprite)
        sprite->Draw();
}

void SunObject::Update()
{
    ClickableObject::Update();

    if (sprite)
    {
        sprite->SetPosition(transform_.x, transform_.y);
        sprite->Update();
    }
}
