#pragma once

#include <functional>

#include "Widget.hpp"

class Button : public Widget
{
public:
    enum class State: std::uint8_t
    {
        Normal,
        Hovered,
        Pressed
    };

    Button() = default;
    void Update() override;

    [[nodiscard]] State GetButtonState() const;
    void SetClickCallback(const std::function<void()>& callback);

protected:
    State state_ = State::Normal;
    std::function<void()> clickCallback_;
    bool wasPressed_ = false;
};
