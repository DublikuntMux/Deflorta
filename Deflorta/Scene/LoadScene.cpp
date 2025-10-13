#include "LoadScene.hpp"

#include "../Resource/ResourceManager.hpp"

LoadScene::LoadScene()
{
    ResourceManager::LoadGroup("Init");
}

void LoadScene::update(float dt)
{
}

void LoadScene::render(Renderer& renderer)
{
}
