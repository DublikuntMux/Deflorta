#include <memory>

#include "Base/Game.hpp"
#include "Base/Input.hpp"
#include "resource.h"
#include "Render/Renderer.hpp"
#include "Scene/LoadScene.hpp"

static LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam)
{
    switch (message)
    {
    case WM_KEYDOWN:
        Input::HandleKeyDown(wParam);
        if (wParam == VK_F1 && Game::IsRunning())
            Renderer::ToggleFPS();
        break;

    case WM_KEYUP:
        Input::HandleKeyUp(wParam);
        break;

    case WM_SIZE:
        if (Game::IsRunning())
            Renderer::Resize(LOWORD(lParam), HIWORD(lParam));
        break;

    case WM_DESTROY:
        PostQuitMessage(0);
        break;

    default:
        return DefWindowProc(hWnd, message, wParam, lParam);
    }
    return 0;
}

static ATOM MyRegisterClass(HINSTANCE hInstance)
{
    const WNDCLASSEXW wcex{
        .cbSize = sizeof(WNDCLASSEX),
        .style = CS_HREDRAW | CS_VREDRAW,
        .lpfnWndProc = WndProc,
        .cbClsExtra = 0,
        .cbWndExtra = 0,
        .hInstance = hInstance,
        .hIcon = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_DEFLORTA)),
        .hCursor = LoadCursor(nullptr, IDC_ARROW),
        .hbrBackground = reinterpret_cast<HBRUSH>(COLOR_WINDOW + 1),
        .lpszMenuName = nullptr,
        .lpszClassName = L"Destolfa",
        .hIconSm = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_SMALL))
    };
    return RegisterClassExW(&wcex);
}

int APIENTRY wWinMain(HINSTANCE hInstance, HINSTANCE, LPWSTR, int nCmdShow)
{
    MyRegisterClass(hInstance);
    const HWND hWnd = CreateWindowW(L"Destolfa", L"Destolfa",
                                    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                                    CW_USEDEFAULT, 0, 1280, 720,
                                    nullptr, nullptr, hInstance, nullptr);

    if (!hWnd) return FALSE;
    ShowWindow(hWnd, nCmdShow);

    Game::Initialize(hWnd);
    Game::SetScene<LoadScene>();
    Game::Run();
    Game::Uninitialize();
    return 0;
}
