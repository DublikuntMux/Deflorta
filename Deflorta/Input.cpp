#include "Input.hpp"

std::unordered_map<WPARAM, bool> Input::keyStates_;

void Input::handleKeyDown(WPARAM key)
{
    keyStates_[key] = true;
}

void Input::handleKeyUp(WPARAM key)
{
    keyStates_[key] = false;
}

bool Input::isKeyPressed(WPARAM key)
{
    return keyStates_.contains(key) && keyStates_[key];
}
