#include "Game.hpp"

#include "Time.hpp"
#include "Input.hpp"
#include "Discord.hpp"
#include "SaveManager.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ResourceManager.hpp"
#include "../Resource/Foley.hpp"

HWND Game::hwnd_;
std::unique_ptr<Scene> Game::scene_;
std::unique_ptr<Scene> Game::next_scene_;
bool Game::running_ = true;

void Game::Initialize(HWND hwnd)
{
    hwnd_ = hwnd;

    SaveManager::Initialize();

    if (!AudioManager::Initialize())
        running_ = false;
    if (!Renderer::Initialize(hwnd_))
        running_ = false;

    ResourceManager::SetRenderBackend(Renderer::GetRenderBackend());

    if (!ResourceManager::LoadManifest())
        running_ = false;

    Foley::LoadManifest();

    Discord::Initialize();
}

void Game::Uninitialize()
{
    SaveManager::Uninitialize();
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
            Input::SetCursorType(IDC_ARROW);
            scene_ = std::move(next_scene_);
            scene_->OnEnter();
        }

        Time::Tick();
        Input::BeginCursorUpdate();

        // Toggle collision debug rendering with F3
        static bool f3WasPressed = false;
        if (Input::IsKeyPressed(VK_F3))
        {
            if (!f3WasPressed && scene_ && scene_->GetCollisionSystem())
            {
                const bool currentState = scene_->GetCollisionSystem()->IsDebugRenderEnabled();
                scene_->GetCollisionSystem()->SetDebugRenderEnabled(!currentState);
            }
            f3WasPressed = true;
        }
        else
        {
            f3WasPressed = false;
        }

        Renderer::BeginFrame();
        if (scene_)
        {
            scene_->Update();
            scene_->Render();
        }
        Input::EndCursorUpdate();
        Renderer::Render();
    }
}

bool Game::IsRunning()
{
    return running_;
}
