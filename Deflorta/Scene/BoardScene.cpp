#include "BoardScene.hpp"

#include "../Base/Discord.hpp"

BoardScene::BoardScene()
{
}

void BoardScene::OnEnter()
{
    Discord::SetPresence("Game", "Play level");
}

void BoardScene::Update()
{
    
}

void BoardScene::Render()
{
    
}
