#include "LoadScene.hpp"

#include "../Resource/ResourceManager.hpp"

LoadScene::LoadScene()
{
    ResourceManager::LoadGroup("Init");
}

void LoadScene::update()
{
}

void LoadScene::render()
{
}
