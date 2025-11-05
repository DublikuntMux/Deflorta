#pragma once

#include <glm/vec2.hpp>

#include <Windows.h>

#include <unordered_map>

class Input
{
public:
    static void HandleKeyDown(WPARAM key);
    static void HandleKeyUp(WPARAM key);
    static bool IsKeyPressed(WPARAM key);

    static void HandleMouseMove(LPARAM lParam);
    static glm::vec2 GetMousePosition();

    static void HandleMouseDown(int vkButton);
    static void HandleMouseUp(int vkButton);
    static bool IsMouseDown(int vkButton);
    static bool IsMousePressed(int vkButton);
    static void ResetMousePresses();

    static void SetCursorType(LPCWSTR idcCursorId);
    static LPCWSTR GetCursorType();
    static void ShowCursor(bool show);
    static bool IsCursorVisible();

    static void BeginCursorUpdate();
    static void EndCursorUpdate();
    static void UpdateCursor();

private:
    static std::unordered_map<WPARAM, bool> keyStates_;

    static glm::vec2 mousePos_;
    static std::unordered_map<int, bool> mouseButtonDown_;
    static std::unordered_map<int, bool> mouseButtonPressed_;
    static LPCWSTR cursorId_;
    static LPCWSTR requestedCursorId_;
    static LPCWSTR activeCursorId_;
    static bool cursorVisible_;
    static bool cursorNeedsUpdate_;
};
