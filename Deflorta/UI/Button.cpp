#include "Button.hpp"

#include "../Base/Input.hpp"
#include "../Render/Renderer.hpp"

#include <cmath>

namespace
{
    bool IsPointOnSegment(float x, float y, const D2D1_POINT_2F& a, const D2D1_POINT_2F& b)
    {
        const float vx = b.x - a.x;
        const float vy = b.y - a.y;
        const float wx = x - a.x;
        const float wy = y - a.y;
        const float cross = vx * wy - vy * wx;
        constexpr float eps = 1e-4f;
        if (std::fabs(cross) > eps) return false;
        const float minX = a.x < b.x ? a.x : b.x;
        const float maxX = a.x > b.x ? a.x : b.x;
        const float minY = a.y < b.y ? a.y : b.y;
        const float maxY = a.y > b.y ? a.y : b.y;
        return x >= minX - eps && x <= maxX + eps && y >= minY - eps && y <= maxY + eps;
    }

    bool PointInPolygon(float x, float y, const std::vector<D2D1_POINT_2F>& poly)
    {
        const size_t n = poly.size();
        if (n < 3) return false;
        for (size_t i = 0, j = n - 1; i < n; j = i++)
        {
            if (IsPointOnSegment(x, y, poly[j], poly[i])) return true;
        }
        bool inside = false;
        for (size_t i = 0, j = n - 1; i < n; j = i++)
        {
            const auto& pi = poly[i];
            const auto& pj = poly[j];
            const bool intersect = pi.y > y != pj.y > y &&
                x < (pj.x - pi.x) * (y - pi.y) / (pj.y - pi.y) + pi.x;
            if (intersect) inside = !inside;
        }
        return inside;
    }
}

void Button::Update()
{
    if (!visible_)
        return;

    const auto [x, y] = Input::GetMousePosition();
    const float floatX = static_cast<float>(x);
    const float floatY = static_cast<float>(y);

    bool mouseOver;
    if (!polygon_.empty() && polygon_.size() >= 3)
    {
        std::vector<D2D1_POINT_2F> worldPolygon;
        worldPolygon.reserve(polygon_.size());
        for (const auto& point : polygon_)
        {
            worldPolygon.push_back({.x = point.x + x_, .y = point.y + y_});
        }
        mouseOver = PointInPolygon(floatX, floatY, worldPolygon);
    }
    else
    {
        mouseOver = floatX >= x_ && floatX <= x_ + width_ &&
            floatY >= y_ && floatY <= y_ + height_;
    }

    if (mouseOver)
    {
        Input::SetCursorType(IDC_HAND);
        if (Input::IsMouseDown(VK_LBUTTON))
        {
            state_ = State::Pressed;
        }
        else
        {
            state_ = State::Hovered;
            if (wasPressed_)
            {
                for (const auto& callback : clickCallback_)
                {
                    callback();
                }
            }
            if (!wasHovered_)
            {
                for (const auto& callback : hoverCallback_)
                {
                    callback();
                }
            }
        }
    }
    else
    {
        state_ = State::Normal;
    }

    wasPressed_ = state_ == State::Pressed;
    wasHovered_ = mouseOver;
}

Button::State Button::GetButtonState() const
{
    return state_;
}

void Button::AddClickCallback(const std::function<void()>& callback)
{
    clickCallback_.push_back(callback);
}

void Button::AddHoverCallback(const std::function<void()>& callback)
{
    hoverCallback_.push_back(callback);
}

void Button::SetPolygon(const std::vector<D2D1_POINT_2F>& polygon)
{
    polygon_ = polygon;
}

void Button::ClearPolygon()
{
    polygon_.clear();
}

bool Button::HasPolygon() const
{
    return !polygon_.empty();
}

void Button::SetRect(float width, float height)
{
    width_ = width;
    height_ = height;
}

void Button::ClearRect()
{
    width_ = 0.0f;
    height_ = 0.0f;
}
