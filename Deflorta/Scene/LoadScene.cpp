#include "LoadScene.hpp"

#include "../Resource/ResourceManager.hpp"

LoadScene::LoadScene()
{
    ResourceManager::LoadGroup("Init");
    ResourceManager::LoadGroup("LoadingImages");
}

void LoadScene::update(float dt)
{
}

void LoadScene::render()
{
}
