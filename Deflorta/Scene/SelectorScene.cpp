#include "SelectorScene.hpp"

#include <stdexcept>

#include "../Base/Random.hpp"
#include "../Render/Reanimator.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Discord.hpp"
#include "../Base/Input.hpp"
#include "../Resource/ResourceManager.hpp"

SelectorScene::SelectorScene()
{
    {
        const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/SelectorScreen.xml");
        if (!reanim.has_value())
            throw std::runtime_error("Failed to load selector scene reanim");

        screenAnimation_ = std::make_unique<Reanimator>(reanim.value());
        screenAnimation_->SetPosition(0, 0);
        screenAnimation_->SetLayerVisible("almanac_key_shadow", false);
        screenAnimation_->PlayLayer("anim_open", ReanimLoopType::PlayOnceAndHold);
        screenAnimation_->SetLayerZ("SelectorScreen_BG", -2);

        grassAnimation_ = std::make_unique<Reanimator>(reanim.value());
        grassAnimation_->SetPosition(0, -30);
        grassAnimation_->PlayLayer("anim_grass", ReanimLoopType::Loop, 6.0f);

        signAnimation_ = std::make_unique<Reanimator>(reanim.value());
        signAnimation_->SetPosition(0, -60);
        signAnimation_->PlayLayer("anim_sign", ReanimLoopType::PlayOnceAndHold);

        cloudAnimation_ = std::make_unique<Reanimator>(reanim.value());
        cloudAnimation_->SetPosition(0, 0);
        const auto cloudId = Random::UniformInt(1, 6);
        cloudAnimation_->PlayLayer("anim_cloud" + std::to_string(cloudId), ReanimLoopType::PlayOnceAndHold, 0.3f);
        for (int i = 1; i < 7; ++i)
        {
            cloudAnimation_->SetLayerZ("Cloud" + std::to_string(i), -1);
        }
    }

    {
        auto button = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_BUTTON");
        auto highlight = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_HIGHLIGHT");
        startButton_ = std::make_unique<ImageButton>(button, highlight, 649.0f, 37.0f, 0.0f, 0.0f);
        startButton_->SetPolygon({
            D2D_POINT_2F{7.0f, 1.0f},
            D2D_POINT_2F{328.0f, 30.0f},
            D2D_POINT_2F{314.0f, 125.0f},
            D2D_POINT_2F{1.0f, 78.0f},
        });
        screenAnimation_->SetLayerVisible("SelectorScreen_Adventure_shadow", false);
    }

    {
        auto button = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_MINIGAME_BUTTON");
        auto highlight = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_MINIGAME_HIGHLIGHT");
        miniGameButton_ = std::make_unique<ImageButton>(button, highlight, 650.0f, 145.0f, 0.0f, 0.0f);
        miniGameButton_->SetPolygon({
            D2D_POINT_2F{7.0f, 1.0f},
            D2D_POINT_2F{267.0f, 62.0f},
            D2D_POINT_2F{257.0f, 124.0f},
            D2D_POINT_2F{7.0f, 57.0f},
        });
    }

    {
        auto button = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_PUZZLE_BUTTON");
        auto highlight = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_PUZZLE_HIGHLIGHT");
        puzzleButton_ = std::make_unique<ImageButton>(button, highlight, 654.0f, 229.4f, 0.0f, 0.0f);
        puzzleButton_->SetPolygon({
            D2D_POINT_2F{2.0f, 0.0f},
            D2D_POINT_2F{281.0f, 55.0f},
            D2D_POINT_2F{268.0f, 121.0f},
            D2D_POINT_2F{3.0f, 60.0f},
        });
    }

    {
        auto button = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON");
        auto highlight = ResourceManager::GetImage("IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_HIGHLIGHT");
        survivalButton_ = std::make_unique<ImageButton>(button, highlight, 657.0f, 300.0f, 0.0f, 0.0f);
        survivalButton_->SetPolygon({
            D2D_POINT_2F{7.0f, 1.0f},
            D2D_POINT_2F{267.0f, 62.0f},
            D2D_POINT_2F{257.0f, 124.0f},
            D2D_POINT_2F{7.0f, 57.0f},
        });
    }

    AudioManager::PlayMusic("resources/sounds/mainmusic.ogg");
    AudioManager::PlaySfx("SOUND_ROLL_IN");
}

void SelectorScene::OnEnter()
{
    Discord::SetPresence("Selector: Opening", "Main Menu");
    Input::SetCursorType(IDC_ARROW);
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
        signAnimation_->PlayLayer("anim_sign_idle", ReanimLoopType::Loop, 3.0f, 2.0f);
    }

    if (cloudAnimation_->IsFinished())
    {
        const auto cloudId = Random::UniformInt(1, 6);
        cloudAnimation_->PlayLayer("anim_cloud" + std::to_string(cloudId), ReanimLoopType::PlayOnceAndHold, 0.3f);
    }

    startButton_->Update();
    miniGameButton_->Update();
    puzzleButton_->Update();
    survivalButton_->Update();
}

void SelectorScene::Render()
{
    screenAnimation_->Draw();
    grassAnimation_->Draw();
    signAnimation_->Draw();
    cloudAnimation_->Draw();

    startButton_->Render();
    miniGameButton_->Render();
    puzzleButton_->Render();
    survivalButton_->Render();
}
