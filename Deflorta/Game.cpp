#include "Game.hpp"

#include <thread>

Game::Game(HWND hwnd) : hwnd_(hwnd)
{
    renderer_ = std::make_unique<Renderer>();

    if (!renderer_->initialize(hwnd_))
        running_ = false;
}

Game::~Game()
{
    if (renderer_)
        renderer_->cleanup();
}

void Game::run()
{
    MSG msg;
    while (running_)
    {
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE))
        {
            if (msg.message == WM_QUIT)
            {
                running_ = false;
                break;
            }
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
        time_.tick();

        renderer_->beginFrame();
        if (scene_)
        {
            scene_->update(time_.deltaTime());
            scene_->render(*renderer_);
        }
        renderer_->render();

        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}

void Game::resize(UINT width, UINT height) const
{
    renderer_->resize(width, height);
}

void Game::toggleFPS() const
{
    renderer_->toggleFPS();
}
