#include "LoadScene.hpp"

#include <iostream>

#include "../Resource/ResourceManager.hpp"
#include "../Render/Renderer.hpp"
#include "../Base/Transform.hpp"
#include "../Resource/TranslationManager.hpp"

namespace
{
    std::atomic g_LoadingDone = false;
}

LoadScene::LoadScene()
{
    ResourceManager::LoadGroup("Init");
    std::thread([]
    {
        TranslationManager::Load("resources/LawnStrings.txt");
        ResourceManager::LoadGroup("LoaderBar");
        ResourceManager::LoadGroup("LoadingImages");
        ResourceManager::LoadGroup("LoadingSounds");
        g_LoadingDone = true;
    }).detach();

    logo_ = ResourceManager::GetImage("IMAGE_POPCAP_LOGO");
    const auto [width, height] = logo_->GetSize();
    logoTransform_ = Transform{
        .x = 1280.0f / 2.0f - width / 2.0f,
        .y = 720.0f / 2.0f - height / 2.0f,
        .rotation = 0.0f,
        .scaleX = 0.5f,
        .scaleY = 0.5f
    };

    const std::vector<TweenProperty> props = {
        {
            .start = 0.5f, .end = 1.2f, .setter = [&](float v)
            {
                logoTransform_.scaleX = v;
                logoTransform_.scaleY = v;
            },
            .mode = TweenMode::EaseOut
        },
        {
            .start = 0.0f, .end = 1.2f, .setter = [&](float v) { logoOpacity_ = v; },
            .mode = TweenMode::EaseIn
        }
    };

    logoTween_ = std::make_unique<Tween>(props, 2.0f);
    logoTween_->start();
}

void LoadScene::update()
{
    logoTween_->update();

    if (g_LoadingDone && !logoTween_->isActive() && !nextTween_)
    {
        const std::vector<TweenProperty> hideProps = {
            {
                .start = logoOpacity_,
                .end = 0.0f,
                .setter = [&](float v) { logoOpacity_ = v; },
                .mode = TweenMode::EaseOut
            },
            {
                .start = logoTransform_.scaleX,
                .end = 1.0f,
                .setter = [&](float v)
                {
                    logoTransform_.scaleX = v;
                    logoTransform_.scaleY = v;
                },
                .mode = TweenMode::EaseIn
            }
        };

        nextTween_ = std::make_unique<Tween>(hideProps, 1.5f);
        nextTween_->start();
    }

    if (nextTween_)
    {
        nextTween_->update();

        if (!nextTween_->isActive())
        {
            std::cout << "All loaded!" << std::endl;
            nextTween_.reset();
        }
    }
}

void LoadScene::render()
{
    Renderer::drawImage(logo_, logoTransform_, logoOpacity_);
}
