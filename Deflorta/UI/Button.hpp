#pragma once

#include <functional>
#include <vector>

#include "Widget.hpp"
#include "../Render/RenderTypes.hpp"

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
    void AddClickCallback(const std::function<void()>& callback);
    void AddHoverCallback(const std::function<void()>& callback);

    void SetRect(float width, float height);
    void ClearRect();

    void SetPolygon(const std::vector<Point2F>& polygon);
    void ClearPolygon();
    [[nodiscard]] bool HasPolygon() const;

protected:
    State state_ = State::Normal;
    std::vector<std::function<void()>> clickCallback_;
    std::vector<std::function<void()>> hoverCallback_;
    bool wasPressed_ = false;
    bool wasHovered_ = false;
    float width_ = 0.0f;
    float height_ = 0.0f;
    std::vector<Point2F> polygon_;
};
