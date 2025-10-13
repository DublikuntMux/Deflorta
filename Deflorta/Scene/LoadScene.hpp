#pragma once

#include <d2d1.h>

#include "Scene.hpp"

class LoadScene final : public Scene
{
public:
    LoadScene();

    void update() override;
    void render() override;

private:
    ID2D1Bitmap* logo_;
};
