#pragma once

#include <glm/vec2.hpp>

struct Rect
{
    glm::vec2 min = {0.0f, 0.0f};
    glm::vec2 max = {0.0f, 0.0f};

    Rect() = default;
    Rect(float l, float t, float r, float b);
    Rect(const glm::vec2& minPoint, const glm::vec2& maxPoint);

    static Rect FromXYWH(float x, float y, float width, float height);
    static Rect FromMinMax(const glm::vec2& minPoint, const glm::vec2& maxPoint);

    [[nodiscard]] bool Contains(const glm::vec2& point) const;
    [[nodiscard]] bool Intersects(const Rect& other) const;

    [[nodiscard]] float Width() const { return max.x - min.x; }
    [[nodiscard]] float Height() const { return max.y - min.y; }
    [[nodiscard]] float X() const { return min.x; }
    [[nodiscard]] float Y() const { return min.y; }

    [[nodiscard]] float Left() const { return min.x; }
    [[nodiscard]] float Top() const { return min.y; }
    [[nodiscard]] float Right() const { return max.x; }
    [[nodiscard]] float Bottom() const { return max.y; }
    [[nodiscard]] glm::vec2 Center() const { return (min + max) * 0.5f; }
    [[nodiscard]] glm::vec2 Size() const { return max - min; }
};
