#include "SelectorScene.hpp"

#include "../Render/Reanimator.hpp"
#include "../Resource/ReanimationLoader.hpp"

SelectorScene::SelectorScene()
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/Blover.xml");
    if (reanim.has_value())
    {
        blover_ = std::make_unique<Reanimator>(reanim.value());
        blover_->PlayLayer("anim_idle");
        blover_->SetPosition(1280.0f / 2.0f, 720.0f / 2.0f);
    }
}

void SelectorScene::Update()
{
    if (blover_)
        blover_->Update();
}

void SelectorScene::Render()
{
    if (blover_)
        blover_->Draw();
}
