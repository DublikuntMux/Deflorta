#pragma once

#include "Scene.hpp"
#include "../Base/Transform.hpp"
#include "../Base/Tween.hpp"
#include "../Render/IRenderBackend.hpp"

#include <memory>

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

    std::shared_ptr<ITexture> screen_;
    std::shared_ptr<ITexture> logo_;
    std::shared_ptr<ITexture> pvzLogo_;
    std::shared_ptr<ITexture> rollCap_;
};
