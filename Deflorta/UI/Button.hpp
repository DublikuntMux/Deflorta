#pragma once

#include <functional>
#include <vector>

#include <d2d1.h>

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

    void SetRect(float width, float height);
    void ClearRect();

    void SetPolygon(const std::vector<D2D1_POINT_2F>& polygon);
    void ClearPolygon();
    [[nodiscard]] bool HasPolygon() const;

    static void SetDebugDrawEnabled(bool enabled);
    static bool IsDebugDrawEnabled();

protected:
    State state_ = State::Normal;
    std::function<void()> clickCallback_;
    bool wasPressed_ = false;
    float width_ = 0.0f;
    float height_ = 0.0f;
    std::vector<D2D1_POINT_2F> polygon_;
};
