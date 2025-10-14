#pragma once

#include <memory>

#include <Windows.h>

#include "../Scene/Scene.hpp"

class Game final
{
public:
    static void Initialize(HWND hwnd);
    static void Uninitialize();

    static void Run();
    static bool IsRunning();

    template <typename T, typename... Args>
    static void SetScene(Args&&... args)
    {
        next_scene_ = std::make_unique<T>(std::forward<Args>(args)...);
    }

private:
    static HWND hwnd_;
    static std::unique_ptr<Scene> scene_;
    static std::unique_ptr<Scene> next_scene_;
    static bool running_;
};
