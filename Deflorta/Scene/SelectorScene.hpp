#pragma once

#include "Scene.hpp"
#include "../Render/Reanimator.hpp"
#include "../UI/ImageButton.hpp"

#include <cstdint>
#include <memory>

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

    std::unique_ptr<ImageButton> optionsButton_;
    std::unique_ptr<ImageButton> helpButton_;
    std::unique_ptr<ImageButton> quitButton_;

    bool startPressed_ = false;
};
