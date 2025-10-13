#include "Game.hpp"

#include <thread>

#include "Time.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ResourceManager.hpp"

Game::Game(HWND hwnd) : hwnd_(hwnd)
{
    if (!AudioManager::Initialize())
        running_ = false;
    if (!Renderer::initialize(hwnd_))
        running_ = false;
    if (!ResourceManager::LoadManifest("resources.xml"))
        running_ = false;
}

Game::~Game()
{
    AudioManager::Uninitialize();
    Renderer::cleanup();
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

        Time::tick();
        Renderer::beginFrame();
        if (scene_)
        {
            scene_->update();
            scene_->render();
        }
        Renderer::render();

        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}
