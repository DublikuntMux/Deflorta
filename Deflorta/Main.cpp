#include <windows.h>

#include <iostream>

#include "Base/Game.hpp"
#include "Base/Window.hpp"
#include "Scene/LoadScene.hpp"

#ifdef _WIN32
extern "C" {
__declspec(dllexport) unsigned long NvOptimusEnablement = 1;
__declspec(dllexport) int AmdPowerXpressRequestHighPerformance = 1;
}
#endif

int APIENTRY wWinMain(HINSTANCE hInstance, HINSTANCE, LPWSTR, int nCmdShow)
{
    if (!Window::Create(1280, 720, "Deflorta"))
    {
        std::cerr << "Failed to create window\n";
        return 1;
    }

    Game::Initialize();
    Game::SetScene<LoadScene>();
    Game::Run();
    Game::Uninitialize();

    Window::Destroy();
    return 0;
}
