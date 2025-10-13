#pragma once

#include <unordered_map>

#include <Windows.h>

class Input
{
public:
    static void handleKeyDown(WPARAM key);
    static void handleKeyUp(WPARAM key);
    static bool isKeyPressed(WPARAM key);

private:
    static std::unordered_map<WPARAM, bool> keyStates_;
};
