#pragma once

#include <glm/vec2.hpp>

struct Rect
{
    float left = 0.0f;
    float top = 0.0f;
    float right = 0.0f;
    float bottom = 0.0f;

    Rect() = default;
    Rect(float l, float t, float r, float b);
    static Rect FromXYWH(float x, float y, float width, float height);

    [[nodiscard]] bool Contains(const glm::vec2& point) const;
    [[nodiscard]] bool Intersects(const Rect& other) const;

    [[nodiscard]] float Width() const { return right - left; }
    [[nodiscard]] float Height() const { return bottom - top; }
    [[nodiscard]] float X() const { return left; }
    [[nodiscard]] float Y() const { return top; }
};
