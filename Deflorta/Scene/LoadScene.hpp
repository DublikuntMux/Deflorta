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
    Transform logoTransform_;
    Transform rollCapTransform_;
    
    float logoOpacity_ = 0.0f;
    float rollCapOpacity_ = 0.0f;

    std::unique_ptr<Tween> startTween_;
    std::unique_ptr<Tween> exitTween_;

    ID2D1Bitmap* rollCap_ = nullptr;
    ID2D1Bitmap* logo_ = nullptr;
};
