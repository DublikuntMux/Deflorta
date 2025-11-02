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

    void OnEnter() override;
    void Update() override;
    void Render() override;

private:
    Transform screenTransform_;
    Transform logoTransform_;
    Transform pvzTransform_;
    Transform rollCapTransform_;

    float logoOpacity_ = 0.0f;

    std::unique_ptr<Tween> startTween_;
    std::unique_ptr<Tween> exitTween_;

    ID2D1Bitmap* screen_ = nullptr;
    ID2D1Bitmap* logo_ = nullptr;
    ID2D1Bitmap* pvzLogo_ = nullptr;
    ID2D1Bitmap* rollCap_ = nullptr;
};
