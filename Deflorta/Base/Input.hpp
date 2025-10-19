#pragma once

#include <unordered_map>

#include <Windows.h>

class Input
{
public:
    static void HandleKeyDown(WPARAM key);
    static void HandleKeyUp(WPARAM key);
    static bool IsKeyPressed(WPARAM key);

    static void HandleMouseMove(LPARAM lParam);
    static int GetMouseX();
    static int GetMouseY();
    static POINT GetMousePosition();

    static void HandleMouseDown(int vkButton);
    static void HandleMouseUp(int vkButton);
    static bool IsMouseDown(int vkButton);
    static bool IsMousePressed(int vkButton);
    static void ResetMousePresses();

    static void SetCursorType(LPCWSTR idcCursorId);
    static LPCWSTR GetCursorType();
    static void ShowCursor(bool show);
    static bool IsCursorVisible();

private:
    static std::unordered_map<WPARAM, bool> keyStates_;

    static POINT mousePos_;
    static std::unordered_map<int, bool> mouseButtonDown_;
    static std::unordered_map<int, bool> mouseButtonPressed_;
    static LPCWSTR cursorId_;
    static bool cursorVisible_;
};
