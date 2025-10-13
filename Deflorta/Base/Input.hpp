#pragma once

#include <unordered_map>

#include <Windows.h>

class Input
{
public:
    static void HandleKeyDown(WPARAM key);
    static void HandleKeyUp(WPARAM key);
    static bool IsKeyPressed(WPARAM key);

private:
    static std::unordered_map<WPARAM, bool> keyStates_;
};
