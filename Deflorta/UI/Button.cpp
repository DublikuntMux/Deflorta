#include "Button.hpp"

#include "../Base/Input.hpp"
#include "../Render/Renderer.hpp"

namespace
{
    // Efficient point-in-polygon test using ray casting algorithm
    bool PointInPolygon(float x, float y, const std::vector<Point2F>& poly)
    {
        const size_t n = poly.size();
        if (n < 3) return false;

        bool inside = false;
        float p1x = poly[0].x;
        float p1y = poly[0].y;

        for (size_t i = 1; i <= n; ++i)
        {
            const float p2x = poly[i % n].x;
            const float p2y = poly[i % n].y;

            if (y > std::min(p1y, p2y))
            {
                if (y <= std::max(p1y, p2y))
                {
                    if (x <= std::max(p1x, p2x))
                    {
                        float xinters;
                        if (p1y != p2y)
                        {
                            xinters = (y - p1y) * (p2x - p1x) / (p2y - p1y) + p1x;
                        }
                        else
                        {
                            xinters = p1x;
                        }

                        if (p1x == p2x || x <= xinters)
                        {
                            inside = !inside;
                        }
                    }
                }
            }

            p1x = p2x;
            p1y = p2y;
        }

        return inside;
    }
}

void Button::Update()
{
    if (!visible_)
        return;

    const auto [x, y] = Input::GetMousePosition();
    const float mouseX = static_cast<float>(x);
    const float mouseY = static_cast<float>(y);

    bool mouseOver;
    if (!polygon_.empty() && polygon_.size() >= 3)
    {
        std::vector<Point2F> worldPolygon;
        worldPolygon.reserve(polygon_.size());
        for (const auto& point : polygon_)
        {
            worldPolygon.emplace_back(point.x + x_, point.y + y_);
        }
        mouseOver = PointInPolygon(mouseX, mouseY, worldPolygon);
    }
    else
    {
        mouseOver = mouseX >= x_ && mouseX <= x_ + width_ &&
            mouseY >= y_ && mouseY <= y_ + height_;
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

void Button::SetPolygon(const std::vector<Point2F>& polygon)
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
