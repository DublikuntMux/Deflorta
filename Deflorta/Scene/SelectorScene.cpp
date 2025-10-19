#include "SelectorScene.hpp"

#include <stdexcept>

#include "../Base/Random.hpp"
#include "../Render/Reanimator.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Discord.hpp"

SelectorScene::SelectorScene()
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/SelectorScreen.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load selector scene reanim");

    {
        screenAnimation_ = std::make_unique<Reanimator>(reanim.value());
        screenAnimation_->SetPosition(0, 0);
        screenAnimation_->SetLayerVisible("almanac_key_shadow", false);
        screenAnimation_->PlayLayer("anim_open", ReanimLoopType::PlayOnceAndHold);
        screenAnimation_->SetLayerZ("SelectorScreen_BG", -2);
    }
    {
        grassAnimation_ = std::make_unique<Reanimator>(reanim.value());
        grassAnimation_->SetPosition(0, -30);
        grassAnimation_->PlayLayer("anim_grass", ReanimLoopType::Loop, 6.0f);
    }
    {
        signAnimation_ = std::make_unique<Reanimator>(reanim.value());
        signAnimation_->SetPosition(0, -60);
        signAnimation_->PlayLayer("anim_sign", ReanimLoopType::PlayOnceAndHold);
    }
    {
        cloudAnimation_ = std::make_unique<Reanimator>(reanim.value());
        cloudAnimation_->SetPosition(0, 0);
        const auto cloudId = Random::UniformInt(1, 6);
        cloudAnimation_->PlayLayer("anim_cloud" + std::to_string(cloudId), ReanimLoopType::PlayOnceAndHold, 0.3f);
        for (int i = 1; i < 7; ++i)
        {
            cloudAnimation_->SetLayerZ("Cloud" + std::to_string(i), -1);
        }
    }

    AudioManager::PlayMusic("resources/sounds/mainmusic.ogg");
    AudioManager::PlaySfx("SOUND_ROLL_IN");
}

void SelectorScene::OnEnter()
{
    Discord::SetPresence("Selector: Opening", "Main Menu");
}

void SelectorScene::Update()
{
    screenAnimation_->Update();
    grassAnimation_->Update();
    signAnimation_->Update();
    cloudAnimation_->Update();

    if (sceneState_ == SelectorState::Open && screenAnimation_->IsFinished())
    {
        sceneState_ = SelectorState::Idle;
        Discord::SetPresence("Main Menu", "Idle");
    }

    if (cloudAnimation_->IsFinished())
    {
        const auto cloudId = Random::UniformInt(1, 6);
        cloudAnimation_->PlayLayer("anim_cloud" + std::to_string(cloudId), ReanimLoopType::PlayOnceAndHold, 0.3f);
    }
}

void SelectorScene::Render()
{
    screenAnimation_->Draw();
    grassAnimation_->Draw();
    signAnimation_->Draw();
    cloudAnimation_->Draw();
}
