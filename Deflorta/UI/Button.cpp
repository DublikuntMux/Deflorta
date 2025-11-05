#include "Button.hpp"

#include "../Base/Input.hpp"
#include "../Render/Renderer.hpp"

namespace
{
    bool PointInPolygon(const glm::vec2& point, const std::vector<glm::vec2>& poly)
    {
        const size_t n = poly.size();
        if (n < 3) return false;

        bool allPositive = true;
        bool allNegative = true;

        for (size_t i = 0; i < n; ++i)
        {
            const glm::vec2 edge = poly[(i + 1) % n] - poly[i];
            const glm::vec2 toPoint = point - poly[i];
            const float cross = glm::cross(glm::vec3(edge, 0.0f), glm::vec3(toPoint, 0.0f)).z;

            if (cross < 0) allPositive = false;
            if (cross > 0) allNegative = false;

            if (!allPositive && !allNegative) return false;
        }

        return true;
    }

    bool PointInRect(const glm::vec2& point, const glm::vec2& rectPos, const glm::vec2& rectSize)
    {
        return glm::all(glm::greaterThanEqual(point, rectPos)) &&
            glm::all(glm::lessThanEqual(point, rectPos + rectSize));
    }
}

void Button::Update()
{
    if (!visible_)
        return;

    const auto mousePos = Input::GetMousePosition();

    bool mouseOver;
    if (!polygon_.empty() && polygon_.size() >= 3)
    {
        std::vector<glm::vec2> worldPolygon;
        worldPolygon.reserve(polygon_.size());
        for (const auto& point : polygon_)
        {
            worldPolygon.emplace_back(point + position_);
        }
        mouseOver = PointInPolygon(mousePos, worldPolygon);
    }
    else
    {
        mouseOver = PointInRect(mousePos, position_, dimensions_);
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

void Button::SetPolygon(const std::vector<glm::vec2>& polygon)
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
    dimensions_ = {width, height};
}

void Button::ClearRect()
{
    dimensions_ = {0.0f, 0.0f};
}
