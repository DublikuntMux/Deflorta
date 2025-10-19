#include "Game.hpp"

#include <thread>

#include "Time.hpp"
#include "Discord.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ResourceManager.hpp"

HWND Game::hwnd_;
std::unique_ptr<Scene> Game::scene_;
std::unique_ptr<Scene> Game::next_scene_;
bool Game::running_ = true;

void Game::Initialize(HWND hwnd)
{
    hwnd_ = hwnd;

    if (!AudioManager::Initialize())
        running_ = false;
    if (!Renderer::Initialize(hwnd_))
        running_ = false;
    if (!ResourceManager::LoadManifest("resources/resources.xml"))
        running_ = false;

    Discord::Initialize();
}

void Game::Uninitialize()
{
    Discord::Shutdown();
    AudioManager::Uninitialize();
    Renderer::Cleanup();
}

void Game::Run()
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

        Discord::Update();

        if (next_scene_)
        {
            if (scene_) scene_->OnExit();
            scene_ = std::move(next_scene_);
            scene_->OnEnter();
        }

        Time::Tick();
        Renderer::BeginFrame();
        if (scene_)
        {
            scene_->Update();
            scene_->Render();
        }
        Renderer::Render();

        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}

bool Game::IsRunning()
{
    return running_;
}
