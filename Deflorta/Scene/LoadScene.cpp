#include "LoadScene.hpp"

#include "../Resource/ResourceManager.hpp"
#include "../Render/Renderer.hpp"
#include "../Base/Transform.hpp"

LoadScene::LoadScene()
{
    ResourceManager::LoadGroup("Init");

    logo_ = ResourceManager::GetImage("IMAGE_POPCAP_LOGO");
}

void LoadScene::update()
{
}

void LoadScene::render()
{
    const auto [width, height] = logo_->GetSize();
    const Transform transform {
        .x = 1280.0f/2.0f - width / 2.0f,
        .y = 720.0f/2.0f - height / 2.0f,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
    };
    Renderer::drawImage(logo_, transform);
}
