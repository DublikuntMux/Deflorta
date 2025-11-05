#include "Rect.hpp"

#include <glm/geometric.hpp>

Rect::Rect(float l, float t, float r, float b) : min(l, t), max(r, b)
{
}

Rect::Rect(const glm::vec2& minPoint, const glm::vec2& maxPoint) : min(minPoint), max(maxPoint)
{
}

Rect Rect::FromXYWH(float x, float y, float width, float height)
{
    return {glm::vec2(x, y), glm::vec2(x + width, y + height)};
}

Rect Rect::FromMinMax(const glm::vec2& minPoint, const glm::vec2& maxPoint)
{
    return {minPoint, maxPoint};
}

bool Rect::Contains(const glm::vec2& point) const
{
    return glm::all(glm::greaterThanEqual(point, min)) &&
        glm::all(glm::lessThanEqual(point, max));
}

bool Rect::Intersects(const Rect& other) const
{
    return min.x < other.max.x &&
        max.x > other.min.x &&
        min.y < other.max.y &&
        max.y > other.min.y;
}
