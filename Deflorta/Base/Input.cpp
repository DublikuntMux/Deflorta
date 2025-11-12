#include "Input.hpp"
#include <GLFW/glfw3.h>
#include <ranges>

std::unordered_map<uint32_t, bool> Input::keyStates_;

glm::vec2 Input::mousePos_{0, 0};
std::unordered_map<uint32_t, bool> Input::mouseButtonDown_{};
std::unordered_map<uint32_t, bool> Input::mouseButtonPressed_{};
int Input::cursorId_ = GLFW_ARROW_CURSOR;
int Input::requestedCursorId_ = GLFW_ARROW_CURSOR;
int Input::activeCursorId_ = GLFW_ARROW_CURSOR;
bool Input::cursorVisible_ = true;
bool Input::cursorNeedsUpdate_ = false;

void Input::HandleKeyDown(Key key)
{
    keyStates_[static_cast<uint32_t>(key)] = true;
}

void Input::HandleKeyUp(Key key)
{
    keyStates_[static_cast<uint32_t>(key)] = false;
}

bool Input::IsKeyPressed(Key key)
{
    const uint32_t keyCode = static_cast<uint32_t>(key);
    return keyStates_.contains(keyCode) && keyStates_[keyCode];
}

void Input::HandleMouseMove(glm::vec2 position)
{
    mousePos_ = position;
}

glm::vec2 Input::GetMousePosition()
{
    return mousePos_;
}

void Input::HandleMouseDown(MouseButton button)
{
    const uint32_t buttonCode = static_cast<uint32_t>(button);
    mouseButtonDown_[buttonCode] = true;
    mouseButtonPressed_[buttonCode] = true;
}

void Input::HandleMouseUp(MouseButton button)
{
    const uint32_t buttonCode = static_cast<uint32_t>(button);
    mouseButtonDown_[buttonCode] = false;
}

bool Input::IsMouseDown(MouseButton button)
{
    const uint32_t buttonCode = static_cast<uint32_t>(button);
    return mouseButtonDown_.contains(buttonCode) && mouseButtonDown_[buttonCode];
}

bool Input::IsMousePressed(MouseButton button)
{
    const uint32_t buttonCode = static_cast<uint32_t>(button);
    return mouseButtonPressed_.contains(buttonCode) && mouseButtonPressed_[buttonCode];
}

void Input::ResetMousePresses()
{
    for (auto& pressed : mouseButtonPressed_ | std::views::values)
        pressed = false;
}

void Input::SetCursorType(int cursorType)
{
    requestedCursorId_ = cursorType;
}

int Input::GetCursorType()
{
    return cursorId_;
}

void Input::ShowCursor(bool show)
{
    if (cursorVisible_ == show)
        return;

    cursorVisible_ = show;
    if (GLFWwindow* window = Window::GetNativeWindow())
    {
        glfwSetInputMode(window, GLFW_CURSOR, show ? GLFW_CURSOR_NORMAL : GLFW_CURSOR_HIDDEN);
    }
}

bool Input::IsCursorVisible()
{
    return cursorVisible_;
}

void Input::BeginCursorUpdate()
{
    requestedCursorId_ = GLFW_ARROW_CURSOR;
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
        if (GLFWwindow* window = Window::GetNativeWindow(); window && cursorVisible_)
        {
            if (GLFWcursor* cursor = glfwCreateStandardCursor(cursorId_))
            {
                glfwSetCursor(window, cursor);
            }
        }
        activeCursorId_ = cursorId_;
        cursorNeedsUpdate_ = false;
    }
}
