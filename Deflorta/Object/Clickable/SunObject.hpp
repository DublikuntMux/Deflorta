#pragma once

#include "ClickableObject.hpp"
#include "SpawnAnimation.hpp"
#include "../../Base/Timer.hpp"
#include "../../Base/Tween.hpp"
#include "../../Render/Reanimator.hpp"

class SunObject : public ClickableObject
{
public:
    SunObject(glm::vec2 position, int value, SpawnAnimation spawnAnimation = SpawnAnimation::None);

    void Render() override;
    void Update() override;

    void OnMouseClick() override;

    int GetValue() const;

private:
    void StartBlinking();
    void StartDisappearing();
    void InitializeSpawnAnimation(SpawnAnimation spawnAnimation);

    std::unique_ptr<Reanimator> sprite_;
    std::unique_ptr<Timer> disappearTimer_;
    std::unique_ptr<Timer> blinkTimer_;
    std::unique_ptr<Tween> blinkTween_;
    std::unique_ptr<Tween> moveTween_;
    std::unique_ptr<Tween> disappearTween_;
    std::unique_ptr<Tween> spawnTween_;
    std::unique_ptr<Tween> spawnMoveTween_;

    bool isBlinkIn_ = true;

    int value_;
};
