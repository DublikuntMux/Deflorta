#pragma once

#include "Scene.hpp"

class BoardScene final : public Scene
{
public:
    BoardScene();

    void OnEnter() override;
    void Update() override;
    void Render() override;
};
