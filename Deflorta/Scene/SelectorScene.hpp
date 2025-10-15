#pragma once

#include <cstdint>

#include "Scene.hpp"

enum class SelectorState: std::uint8_t
{
    Open,
    Idle,
};

class SelectorScene final : public Scene
{
public:
    SelectorScene();

    void Update() override;
    void Render() override;

private:
    SelectorState sceneState_ = SelectorState::Open;
};
