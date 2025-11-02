#include "LoadScene.hpp"

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
                            float scale = 1.0f) -> std::pair<ID2D1Bitmap*, Transform>
    {
        ID2D1Bitmap* img = ResourceManager::GetImage(name);
        if (!img)
            return {nullptr, Transform{.x = centerX, .y = centerY, .rotation = 0.0f, .scaleX = scale, .scaleY = scale}};
        D2D1_SIZE_F size = img->GetSize();
        Transform t{
            .x = centerX - size.width / 2.0f, .y = centerY - size.height / 2.0f, .rotation = 0.0f, .scaleX = scale,
            .scaleY = scale
        };
        return {img, t};
    };

    std::tie(screen_, screenTransform_) = loadCentered("IMAGE_TITLESCREEN", 1280.0f / 2.0f, 720.0f / 2.0f);
    std::tie(pvzLogo_, pvzTransform_) = loadCentered("IMAGE_PVZ_LOGO", 1280.0f / 2.0f, 100.0f);
    auto logoPair = loadCentered("IMAGE_POPCAP_LOGO", 1280.0f / 2.0f, 720.0f / 2.0f);
    logo_ = logoPair.first;
    logoTransform_ = logoPair.second;
    logoTransform_.scaleX = logoTransform_.scaleY = 0.5f;

    auto rollPair = loadCentered("IMAGE_REANIM_SODROLLCAP", 1280.0f / 2.0f, 720.0f / 2.0f + 200.0f);
    rollCap_ = rollPair.first;
    rollCapTransform_ = rollPair.second;

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
        }
    };

    startTween_ = std::make_unique<Tween>(props, 2.0f);
    startTween_->Start();
}

void LoadScene::OnEnter()
{
    Discord::SetPresence("Loading", "Starting up");
    Input::SetCursorType(IDC_WAIT);
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
    Renderer::EnqueueImage(screen_, screenTransform_);
    Renderer::EnqueueImage(logo_, logoTransform_, logoOpacity_);
    Renderer::EnqueueImage(pvzLogo_, pvzTransform_, logoOpacity_);
    Renderer::EnqueueImage(rollCap_, rollCapTransform_, logoOpacity_);
}
