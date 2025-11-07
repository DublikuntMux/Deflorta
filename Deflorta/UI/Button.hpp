#pragma once

#include "Widget.hpp"

#include <glm/glm.hpp>

#include <functional>
#include <vector>

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
    void AddClickCallback(std::function<void()> callback);
    void AddHoverCallback(std::function<void()> callback);

    void SetRect(float width, float height);
    void ClearRect();

    void SetPolygon(std::vector<glm::vec2> polygon);
    void ClearPolygon();
    [[nodiscard]] bool HasPolygon() const;

protected:
    State state_ = State::Normal;
    std::vector<std::function<void()>> clickCallback_;
    std::vector<std::function<void()>> hoverCallback_;
    bool wasPressed_ = false;
    bool wasHovered_ = false;
    glm::vec2 dimensions_;
    std::vector<glm::vec2> polygon_;
};
