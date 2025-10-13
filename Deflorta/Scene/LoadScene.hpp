#pragma once

#include "Scene.hpp"

class LoadScene final: public Scene
{
public:
    LoadScene();

    void update(float dt) override;
    void render(Renderer& renderer) override;
};
