#pragma once

#include "Window.hpp"
#include <glm/vec2.hpp>
#include <unordered_map>

class Input
{
public:
    static void HandleKeyDown(Key key);
    static void HandleKeyUp(Key key);
    static bool IsKeyPressed(Key key);

    static void HandleMouseMove(glm::vec2 position);
    static glm::vec2 GetMousePosition();

    static void HandleMouseDown(MouseButton button);
    static void HandleMouseUp(MouseButton button);
    static bool IsMouseDown(MouseButton button);
    static bool IsMousePressed(MouseButton button);
    static void ResetMousePresses();

    static void SetCursorType(int cursorType);
    static int GetCursorType();
    static void ShowCursor(bool show);
    static bool IsCursorVisible();

    static void BeginCursorUpdate();
    static void EndCursorUpdate();
    static void UpdateCursor();

private:
    static std::unordered_map<uint32_t, bool> keyStates_;

    static glm::vec2 mousePos_;
    static std::unordered_map<uint32_t, bool> mouseButtonDown_;
    static std::unordered_map<uint32_t, bool> mouseButtonPressed_;
    static int cursorId_;
    static int requestedCursorId_;
    static int activeCursorId_;
    static bool cursorVisible_;
    static bool cursorNeedsUpdate_;
};
