#include "Input.hpp"

std::unordered_map<WPARAM, bool> Input::keyStates_;

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
