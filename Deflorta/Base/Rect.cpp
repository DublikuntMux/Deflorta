#include "Rect.hpp"

Rect::Rect(float l, float t, float r, float b) : left(l), top(t), right(r), bottom(b)
{
}

Rect Rect::FromXYWH(float x, float y, float width, float height)
{
    return {x, y, x + width, y + height};
}

bool Rect::Contains(const glm::vec2& point) const
{
    return point.x >= left && point.x <= right &&
        point.y >= top && point.y <= bottom;
}

bool Rect::Intersects(const Rect& other) const
{
    return left < other.right &&
        right > other.left &&
        top < other.bottom &&
        bottom > other.top;
}
