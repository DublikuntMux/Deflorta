﻿#pragma once

#include <cstdint>
#include <memory>

#include "Scene.hpp"
#include "../Render/Reanimator.hpp"
#include "../UI/ImageButton.hpp"

enum class SelectorState: std::uint8_t
{
    Open,
    Idle,
};

class SelectorScene final : public Scene
{
public:
    SelectorScene();

    void OnEnter() override;
    void Update() override;
    void Render() override;

private:
    SelectorState sceneState_ = SelectorState::Open;

    std::unique_ptr<Reanimator> screenAnimation_;
    std::unique_ptr<Reanimator> grassAnimation_;
    std::unique_ptr<Reanimator> signAnimation_;
    std::unique_ptr<Reanimator> cloudAnimation_;

    std::unique_ptr<ImageButton> startButton_;
    std::unique_ptr<ImageButton> miniGameButton_;
    std::unique_ptr<ImageButton> puzzleButton_;
    std::unique_ptr<ImageButton> survivalButton_;
};
