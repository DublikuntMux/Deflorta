#include "Input.hpp"

#include <Windowsx.h>

#include <ranges>

std::unordered_map<WPARAM, bool> Input::keyStates_;

POINT Input::mousePos_{0, 0};
std::unordered_map<int, bool> Input::mouseButtonDown_{};
std::unordered_map<int, bool> Input::mouseButtonPressed_{};
LPCWSTR Input::cursorId_ = IDC_ARROW;
LPCWSTR Input::requestedCursorId_ = IDC_ARROW;
LPCWSTR Input::activeCursorId_ = IDC_ARROW;
bool Input::cursorVisible_ = true;
bool Input::cursorNeedsUpdate_ = false;

void Input::HandleKeyDown(WPARAM key)
{
    keyStates_[key] = true;
}

void Input::HandleKeyUp(WPARAM key)
{
    keyStates_[key] = false;
}

bool Input::IsKeyPressed(WPARAM key)
{
    return keyStates_.contains(key) && keyStates_[key];
}

void Input::HandleMouseMove(LPARAM lParam)
{
    mousePos_.x = GET_X_LPARAM(lParam);
    mousePos_.y = GET_Y_LPARAM(lParam);
}

int Input::GetMouseX()
{
    return mousePos_.x;
}

int Input::GetMouseY()
{
    return mousePos_.y;
}

POINT Input::GetMousePosition()
{
    return mousePos_;
}

void Input::HandleMouseDown(int vkButton)
{
    mouseButtonDown_[vkButton] = true;
    mouseButtonPressed_[vkButton] = true;
}

void Input::HandleMouseUp(int vkButton)
{
    mouseButtonDown_[vkButton] = false;
}

bool Input::IsMouseDown(int vkButton)
{
    return mouseButtonDown_.contains(vkButton) && mouseButtonDown_[vkButton];
}

bool Input::IsMousePressed(int vkButton)
{
    return mouseButtonPressed_.contains(vkButton) && mouseButtonPressed_[vkButton];
}

void Input::ResetMousePresses()
{
    for (auto& pressed : mouseButtonPressed_ | std::views::values)
        pressed = false;
}

void Input::SetCursorType(LPCWSTR idcCursorId)
{
    requestedCursorId_ = idcCursorId;
}

LPCWSTR Input::GetCursorType()
{
    return cursorId_;
}

void Input::ShowCursor(bool show)
{
    if (cursorVisible_ == show)
        return;

    cursorVisible_ = show;
    ::ShowCursor(show ? TRUE : FALSE);
}

bool Input::IsCursorVisible()
{
    return cursorVisible_;
}

void Input::BeginCursorUpdate()
{
    requestedCursorId_ = IDC_ARROW;
}

void Input::EndCursorUpdate()
{
    if (requestedCursorId_ != cursorId_)
    {
        cursorId_ = requestedCursorId_;
        cursorNeedsUpdate_ = true;
    }
}

void Input::UpdateCursor()
{
    if (cursorNeedsUpdate_)
    {
        if (cursorVisible_)
            SetCursor(LoadCursor(nullptr, cursorId_));
        else
            SetCursor(nullptr);
        activeCursorId_ = cursorId_;
        cursorNeedsUpdate_ = false;
    }
}
