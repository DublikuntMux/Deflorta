#pragma once

#include <memory>

#include <Windows.h>

#include "Renderer.hpp"
#include "../Scene/Scene.hpp"
#include "Time.hpp"

class Game final
{
public:
    explicit Game(HWND hwnd);
    ~Game();

    void run();
    void resize(UINT width, UINT height) const;

    template <typename T, typename... Args>
    void setScene(Args&&... args)
    {
        if (scene_) scene_->onExit();
        scene_ = std::make_unique<T>(std::forward<Args>(args)...);
        scene_->onEnter();
    }

    void toggleFPS() const;

private:
    HWND hwnd_;

    std::unique_ptr<Renderer> renderer_;
    std::unique_ptr<Scene> scene_;

    Time time_;
    bool running_ = true;
};
