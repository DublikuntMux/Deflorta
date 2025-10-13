#pragma once

#include <memory>

#include <Windows.h>

#include "../Scene/Scene.hpp"

class Game final
{
public:
    explicit Game(HWND hwnd);
    ~Game();

    void run();

    template <typename T, typename... Args>
    void setScene(Args&&... args)
    {
        next_scene_ = std::make_unique<T>(std::forward<Args>(args)...);
    }

private:
    HWND hwnd_;
    std::unique_ptr<Scene> scene_;
    std::unique_ptr<Scene> next_scene_;
    bool running_ = true;
};
