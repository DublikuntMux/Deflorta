#include "Game.hpp"

#include <GLFW/glfw3.h>

#include "Time.hpp"
#include "Input.hpp"
#include "Discord.hpp"
#include "SaveManager.hpp"
#include "Window.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/AudioManager.hpp"
#include "../Resource/ResourceManager.hpp"
#include "../Resource/Foley.hpp"

std::unique_ptr<Scene> Game::scene_;
std::unique_ptr<Scene> Game::next_scene_;
bool Game::running_ = true;

void Game::Initialize()
{
    SaveManager::Initialize();

    if (!AudioManager::Initialize())
        running_ = false;
    if (!Renderer::Initialize(Window::GetNativeWindowHandle()))
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
    while (running_ && Window::IsOpen())
    {
        Window::PollEvents();
        Discord::Update();

        if (next_scene_)
        {
            if (scene_) scene_->OnExit();
            Input::SetCursorType(GLFW_ARROW_CURSOR);
            scene_ = std::move(next_scene_);
            scene_->OnEnter();
        }

        Time::Tick();
        Input::BeginCursorUpdate();

        static bool f3WasPressed = false;
        if (Input::IsKeyPressed(Key::F3))
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
        Input::UpdateCursor();
        Renderer::Render();
    }
}

bool Game::IsRunning()
{
    return running_;
}
