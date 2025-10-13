#pragma once

#include "Scene.hpp"

class LoadScene final : public Scene
{
public:
    LoadScene();

    void update() override;
    void render() override;
};
