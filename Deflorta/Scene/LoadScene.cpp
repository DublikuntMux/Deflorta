#include "LoadScene.hpp"

#include <GLFW/glfw3.h>

#include "SelectorScene.hpp"
#include "../Base/Game.hpp"
#include "../Base/Time.hpp"
#include "../Resource/ResourceManager.hpp"
#include "../Render/Renderer.hpp"
#include "../Base/Transform.hpp"
#include "../Resource/Foley.hpp"
#include "../Resource/TranslationManager.hpp"
#include "../Base/Discord.hpp"
#include "../Base/Input.hpp"
#include "../Render/Layer.hpp"

namespace
{
    std::atomic_bool g_LoadingDone = false;
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

    auto loadCentered = [&](const std::string& name, float centerX, float centerY,
                            float scale = 1.0f) -> std::pair<std::shared_ptr<ITexture>, Transform>
    {
        auto img = ResourceManager::GetImage(name);
        if (!img)
            return {nullptr, Transform{.position = {centerX, centerY}, .scale = {scale, scale}, .rotation = 0.0f}};
        const auto size = img->GetSize();
        Transform t{
            .position = {centerX - size.x / 2.0f, centerY - size.y / 2.0f}, .scale = {scale, scale}, .rotation = 0.0f
        };
        return {img, t};
    };

    std::tie(screen_, screenTransform_) = loadCentered("IMAGE_TITLESCREEN", 1280.0f / 2.0f, 720.0f / 2.0f);
    std::tie(pvzLogo_, pvzTransform_) = loadCentered("IMAGE_PVZ_LOGO", 1280.0f / 2.0f, 100.0f);
    const auto logoPair = loadCentered("IMAGE_POPCAP_LOGO", 1280.0f / 2.0f, 720.0f / 2.0f);
    logo_ = logoPair.first;
    logoTransform_ = logoPair.second;
    logoTransform_.scale = {0.5f, 0.5f};

    const auto rollPair = loadCentered("IMAGE_REANIM_SODROLLCAP", 1280.0f / 2.0f, 720.0f / 2.0f + 200.0f);
    rollCap_ = rollPair.first;
    rollCapTransform_ = rollPair.second;

    const std::vector<TweenProperty> props = {
        {
            .start = 0.5f, .end = 1.2f, .setter = [&](float v)
            {
                logoTransform_.scale = {v, v};
            },
            .mode = TweenMode::EaseOut
        },
        {
            .start = 0.0f, .end = 1.0f, .setter = [&](float v) { logoOpacity_ = v; },
            .mode = TweenMode::EaseIn
        }
    };

    startTween_ = std::make_unique<Tween>(props, 2.0f);
    startTween_->Start();
}

void LoadScene::OnEnter()
{
    Discord::SetPresence("Loading", "Starting up");
    Input::SetCursorType(GLFW_NOT_ALLOWED_CURSOR);
}

void LoadScene::Update()
{
    Scene::Update();

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
                .start = logoTransform_.scale.x,
                .end = 1.0f,
                .setter = [&](float v)
                {
                    logoTransform_.scale = {v, v};
                },
                .mode = TweenMode::EaseIn
            }
        };

        exitTween_ = std::make_unique<Tween>(hideProps, 1.5f);
        exitTween_->Start();

        Foley::Play("Sun");
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
    Scene::Render();

    Renderer::EnqueueImage(screen_, screenTransform_, 1.0f, static_cast<int>(RenderLayer::Foreground));
    Renderer::EnqueueImage(logo_, logoTransform_, logoOpacity_, static_cast<int>(RenderLayer::UI));
    Renderer::EnqueueImage(pvzLogo_, pvzTransform_, logoOpacity_, static_cast<int>(RenderLayer::UI));
    Renderer::EnqueueImage(rollCap_, rollCapTransform_, logoOpacity_, static_cast<int>(RenderLayer::UI));
}
