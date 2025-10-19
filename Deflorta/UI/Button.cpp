#include "Button.hpp"

#include "../Base/Input.hpp"

void Button::Update()
{
    if (!visible_)
        return;

    const auto [x, y] = Input::GetMousePosition();
    const float floatX = static_cast<float>(x);
    const float floatY = static_cast<float>(y);
    const bool mouseOver = floatX >= rect_.left && floatX <= rect_.right &&
        floatY >= rect_.top && floatY <= rect_.bottom;

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
                if (clickCallback_)
                {
                    clickCallback_();
                }
            }
        }
    }
    else
    {
        state_ = State::Normal;
        Input::SetCursorType(IDC_ARROW);
    }

    wasPressed_ = state_ == State::Pressed;
}

Button::State Button::GetButtonState() const
{
    return state_;
}

void Button::SetClickCallback(const std::function<void()>& callback)
{
    clickCallback_ = callback;
}
