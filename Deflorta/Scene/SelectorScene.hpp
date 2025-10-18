#pragma once

#include <cstdint>
#include <memory>

#include "Scene.hpp"
#include "../Render/Reanimator.hpp"

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
    std::unique_ptr<Reanimator> blover_;
};
