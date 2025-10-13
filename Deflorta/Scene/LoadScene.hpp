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

    void Update() override;
    void Render() override;

private:
    ID2D1Bitmap* logo_ = nullptr;
    std::unique_ptr<Tween> logoTween_;
    std::unique_ptr<Tween> nextTween_;
    Transform logoTransform_;
    float logoOpacity_ = 0.0f;
};
