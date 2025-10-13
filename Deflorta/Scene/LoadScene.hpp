#pragma once

#include <memory>

#include <d2d1.h>

#include "Scene.hpp"
#include "../Base/Transform.hpp"
#include "../Base/Tween.hpp"

class LoadScene final : public Scene
{
public:
    LoadScene();

    void update() override;
    void render() override;

private:
    ID2D1Bitmap* logo_;
    std::unique_ptr<Tween> logoTween_;
    Transform logoTransform_;
    float logoOpacity_ = 0.0f;
};
