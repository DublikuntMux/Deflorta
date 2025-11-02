#include "BoardScene.hpp"

#include "../Base/Discord.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

BoardScene::BoardScene(BoardSettings settings) : settings_(std::move(settings))
{
    switch (settings_.backgroundType)
    {
    case BackgroundType::Day:
        ResourceManager::LoadGroup("DelayLoad_Background1");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND1");
        cover_ = ResourceManager::GetImage("IMAGE_BACKGROUND1_COVER");
        bushes_ = std::make_unique<Bush>(5, false);
        coverTransform_ = Transform{
            .x = 920,
            .y = 580,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    case BackgroundType::Night:
        ResourceManager::LoadGroup("DelayLoad_Background2");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND2");
        cover_ = ResourceManager::GetImage("IMAGE_BACKGROUND2_COVER");
        bushes_ = std::make_unique<Bush>(5, true);
        coverTransform_ = Transform{
            .x = 920,
            .y = 580,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    case BackgroundType::Pool:
        ResourceManager::LoadGroup("DelayLoad_Background3");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND3");
        cover_ = ResourceManager::GetImage("IMAGE_BACKGROUND3_COVER");
        bushes_ = std::make_unique<Bush>(6, false);
        coverTransform_ = Transform{
            .x = 925,
            .y = 635,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    case BackgroundType::PoolNight:
        ResourceManager::LoadGroup("DelayLoad_Background4");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND4");
        cover_ = ResourceManager::GetImage("IMAGE_BACKGROUND4_COVER");
        bushes_ = std::make_unique<Bush>(6, true);
        coverTransform_ = Transform{
            .x = 925,
            .y = 635,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    case BackgroundType::Roof:
        ResourceManager::LoadGroup("DelayLoad_Background5");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND5");
        pole_ = ResourceManager::GetImage("IMAGE_BACKGROUND5_POLE");
        tree_ = ResourceManager::GetImage("IMAGE_BACKGROUND5_TREES");

        poleTransform_ = Transform{
            .x = 870,
            .y = 0,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        treeTransform_ = Transform{
            .x = 930,
            .y = 0,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    case BackgroundType::RoofNight:
        ResourceManager::LoadGroup("DelayLoad_Background6");
        background_ = ResourceManager::GetImage("IMAGE_BACKGROUND6");
        pole_ = ResourceManager::GetImage("IMAGE_BACKGROUND6_POLE");
        tree_ = ResourceManager::GetImage("IMAGE_BACKGROUND6_TREES");

        poleTransform_ = Transform{
            .x = 870,
            .y = 0,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        treeTransform_ = Transform{
            .x = 930,
            .y = 0,
            .rotation = 0.0f,
            .scaleX = 1.0f,
            .scaleY = 1.0f
        };
        break;
    }

    backgroundTransform_ = Transform{
        .x = -220,
        .y = 0,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
    };

    if (settings_.hasFog)
    {
        const int rows = settings_.backgroundType == BackgroundType::Pool || 
                         settings_.backgroundType == BackgroundType::PoolNight ? 6 : 5;
        fog_ = std::make_unique<Fog>(rows, settings_.fogColumns);
    }
}

void BoardScene::OnEnter()
{
    const auto description = "Play level: " + settings_.levelName;
    Discord::SetPresence("Game", description);
}

void BoardScene::Update()
{
    if (bushes_)
        bushes_->Update();
    if (fog_)
        fog_->Update();
}

void BoardScene::Render()
{
    Renderer::EnqueueImage(background_, backgroundTransform_);
    if (bushes_)
        bushes_->Draw();
    if (cover_)
        Renderer::EnqueueImage(cover_, coverTransform_);
    if (fog_)
        fog_->Render();
    if (tree_)
        Renderer::EnqueueImage(tree_, treeTransform_);
    if (pole_)
        Renderer::EnqueueImage(pole_, poleTransform_);
}
