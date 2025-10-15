#include "LoadScene.hpp"

#include "SelectorScene.hpp"
#include "../Base/Game.hpp"
#include "../Base/Time.hpp"
#include "../Resource/ResourceManager.hpp"
#include "../Render/Renderer.hpp"
#include "../Base/Transform.hpp"
#include "../Resource/Foley.hpp"
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
        ResourceManager::LoadGroup("LoadingImages");
        ResourceManager::LoadGroup("LoadingSounds");
        ResourceManager::LoadGroup("LoadingFonts");
        g_LoadingDone = true;
    }).detach();

    logo_ = ResourceManager::GetImage("IMAGE_POPCAP_LOGO");
    const auto [logoWidth, logoHeight] = logo_->GetSize();
    logoTransform_ = Transform{
        .x = 1280.0f / 2.0f - logoWidth / 2.0f,
        .y = 720.0f / 2.0f - logoHeight / 2.0f,
        .rotation = 0.0f,
        .scaleX = 0.5f,
        .scaleY = 0.5f
    };

    rollCap_ = ResourceManager::GetImage("IMAGE_REANIM_SODROLLCAP");
    const auto [rollWidth, rollHeight] = rollCap_->GetSize();
    rollCapTransform_ = Transform{
        .x = 1280.0f / 2.0f - rollWidth / 2.0f,
        .y = 720.0f / 2.0f + 200.0f - rollHeight / 2.0f,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
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
            .start = 0.0f, .end = 1.0f, .setter = [&](float v) { logoOpacity_ = v; },
            .mode = TweenMode::EaseIn
        },
        {
            .start = 0.0f, .end = 1.0f, .setter = [&](float v) { rollCapOpacity_ = v; },
            .mode = TweenMode::EaseIn
        }
    };

    startTween_ = std::make_unique<Tween>(props, 2.0f);
    startTween_->Start();
}

void LoadScene::Update()
{
    startTween_->Update();
    rollCapTransform_.rotation += 90.0f * Time::GetDeltaTime();

    if (g_LoadingDone && !startTween_->IsActive() && !exitTween_)
    {
        const std::vector<TweenProperty> hideProps = {
            {
                .start = logoOpacity_,
                .end = 0.0f,
                .setter = [&](float v) { logoOpacity_ = v; },
                .mode = TweenMode::EaseOut
            },
            {
                .start = rollCapOpacity_,
                .end = 0.0f,
                .setter = [&](float v) { rollCapOpacity_ = v; },
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

        exitTween_ = std::make_unique<Tween>(hideProps, 1.5f);
        exitTween_->Start();

        Foley::Play(FoleyType::Sun);
    }

    if (exitTween_)
    {
        exitTween_->Update();

        if (!exitTween_->IsActive())
        {
            Game::SetScene<SelectorScene>();
            exitTween_.reset();
        }
    }
}

void LoadScene::Render()
{
    Renderer::DrawImage(logo_, logoTransform_, logoOpacity_);
    Renderer::DrawImage(rollCap_, rollCapTransform_, rollCapOpacity_);
}
