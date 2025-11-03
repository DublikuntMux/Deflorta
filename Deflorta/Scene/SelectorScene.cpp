#include "SelectorScene.hpp"

#include <stdexcept>

#include "BoardScene.hpp"
#include "../Base/Random.hpp"
#include "../Render/Reanimator.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Discord.hpp"
#include "../Base/Game.hpp"
#include "../Resource/Foley.hpp"
#include "../Resource/ResourceManager.hpp"

SelectorScene::SelectorScene()
{
    auto onPressGrave = []
    {
        AudioManager::PlaySfx("SOUND_GRAVEBUTTON");
    };
    auto onPress = []
    {
        AudioManager::PlaySfx("SOUND_TAP");
    };
    auto onHover = []
    {
        Foley::Play("Bleep");
    };

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

    auto makeImageButton = [&](const std::string& imgName, const std::string& hlName, float x, float y, float hotX,
                               float hotY,
                               const std::function<void(ImageButton*)>& extraConfig = {}) -> std::unique_ptr<
        ImageButton>
    {
        auto buttonImg = ResourceManager::GetImage(imgName);
        auto highlight = ResourceManager::GetImage(hlName);
        auto btn = std::make_unique<ImageButton>(buttonImg, highlight, x, y, hotX, hotY);
        if (extraConfig) extraConfig(btn.get());
        return btn;
    };

    {
        startButton_ = makeImageButton("IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_BUTTON",
                                       "IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_HIGHLIGHT",
                                       649.0f, 37.0f, 0.0f, 0.0f,
                                       [&](ImageButton* b)
                                       {
                                           b->SetPolygon({
                                               D2D_POINT_2F{7.0f, 1.0f},
                                               D2D_POINT_2F{328.0f, 30.0f},
                                               D2D_POINT_2F{314.0f, 125.0f},
                                               D2D_POINT_2F{1.0f, 78.0f},
                                           });
                                           b->AddHoverCallback(onHover);
                                           b->AddClickCallback(onPressGrave);
                                           b->AddClickCallback([]
                                           {
                                               const auto settings = BoardSettings{
                                                   .levelName = "1-1",
                                                   .backgroundType = BackgroundType::Day,
                                                   .hasFog = false,
                                                   .fogColumns = 0,
                                               };
                                               Game::SetScene<BoardScene>(settings);
                                           });
                                       });
        AddWidget(startButton_.get());
    }

    {
        miniGameButton_ = makeImageButton("IMAGE_REANIM_SELECTORSCREEN_MINIGAME_BUTTON",
                                          "IMAGE_REANIM_SELECTORSCREEN_MINIGAME_HIGHLIGHT",
                                          650.0f, 145.0f, 0.0f, 0.0f,
                                          [&](ImageButton* b)
                                          {
                                              b->SetPolygon({
                                                  D2D_POINT_2F{7.0f, 1.0f},
                                                  D2D_POINT_2F{267.0f, 62.0f},
                                                  D2D_POINT_2F{257.0f, 124.0f},
                                                  D2D_POINT_2F{7.0f, 57.0f},
                                              });
                                              b->AddHoverCallback(onHover);
                                              b->AddClickCallback(onPressGrave);
                                          });
        AddWidget(miniGameButton_.get());
    }

    {
        puzzleButton_ = makeImageButton("IMAGE_REANIM_SELECTORSCREEN_PUZZLE_BUTTON",
                                        "IMAGE_REANIM_SELECTORSCREEN_PUZZLE_HIGHLIGHT",
                                        654.0f, 229.4f, 0.0f, 0.0f,
                                        [&](ImageButton* b)
                                        {
                                            b->SetPolygon({
                                                D2D_POINT_2F{2.0f, 0.0f},
                                                D2D_POINT_2F{281.0f, 55.0f},
                                                D2D_POINT_2F{268.0f, 121.0f},
                                                D2D_POINT_2F{3.0f, 60.0f},
                                            });
                                            b->AddHoverCallback(onHover);
                                            b->AddClickCallback(onPressGrave);
                                        });
        AddWidget(puzzleButton_.get());
    }

    {
        survivalButton_ = makeImageButton("IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON",
                                          "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_HIGHLIGHT",
                                          657.0f, 300.0f, 0.0f, 0.0f,
                                          [&](ImageButton* b)
                                          {
                                              b->SetPolygon({
                                                  D2D_POINT_2F{7.0f, 1.0f},
                                                  D2D_POINT_2F{267.0f, 62.0f},
                                                  D2D_POINT_2F{257.0f, 124.0f},
                                                  D2D_POINT_2F{7.0f, 57.0f},
                                              });
                                              b->AddHoverCallback(onHover);
                                              b->AddClickCallback(onPressGrave);
                                          });
        AddWidget(survivalButton_.get());
    }

    {
        optionsButton_ = makeImageButton("IMAGE_SELECTORSCREEN_OPTIONS_BUTTON",
                                         "IMAGE_SELECTORSCREEN_OPTIONS_HIGHLIGHT",
                                         813.0f, 470.0f, 81.0f, 31.0f,
                                         [&](ImageButton* b)
                                         {
                                             b->AddHoverCallback(onHover);
                                             b->AddClickCallback(onPress);
                                         });
        AddWidget(optionsButton_.get());
    }

    {
        helpButton_ = makeImageButton("IMAGE_SELECTORSCREEN_HELP_BUTTON",
                                      "IMAGE_SELECTORSCREEN_HELP_HIGHLIGHT",
                                      895.0f, 505.0f, 48.0f, 22.0f,
                                      [&](ImageButton* b)
                                      {
                                          b->AddHoverCallback(onHover);
                                          b->AddClickCallback(onPress);
                                      });
        AddWidget(helpButton_.get());
    }

    {
        quitButton_ = makeImageButton("IMAGE_SELECTORSCREEN_QUIT_BUTTON",
                                      "IMAGE_SELECTORSCREEN_QUIT_HIGHLIGHT",
                                      970.0f, 490.0f, 57.0f, 27.0f,
                                      [&](ImageButton* b)
                                      {
                                          b->AddHoverCallback(onHover);
                                          b->AddClickCallback(onPress);
                                      });
        AddWidget(quitButton_.get());
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
    Scene::Update();
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
}

void SelectorScene::Render()
{
    Scene::Render();
    screenAnimation_->Draw();
    grassAnimation_->Draw();
    signAnimation_->Draw();
    cloudAnimation_->Draw();
}
