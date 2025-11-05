#include <iostream>

#include "Base/Game.hpp"
#include "Base/Input.hpp"
#include "resource.h"
#include "Render/Renderer.hpp"
#include "Scene/LoadScene.hpp"
#include "UI/Button.hpp"

constexpr wchar_t WINDOW_CLASS_NAME[] = L"Deflorta";
constexpr wchar_t WINDOW_TITLE[] = L"Deflorta";
constexpr int WINDOW_WIDTH = 1280;
constexpr int WINDOW_HEIGHT = 720;

namespace
{
    LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam)
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

        case WM_MOUSEMOVE:
            Input::HandleMouseMove(lParam);
            break;

        case WM_LBUTTONDOWN:
            Input::HandleMouseDown(VK_LBUTTON);
            break;
        case WM_LBUTTONUP:
            Input::HandleMouseUp(VK_LBUTTON);
            break;
        case WM_RBUTTONDOWN:
            Input::HandleMouseDown(VK_RBUTTON);
            break;
        case WM_RBUTTONUP:
            Input::HandleMouseUp(VK_RBUTTON);
            break;
        case WM_MBUTTONDOWN:
            Input::HandleMouseDown(VK_MBUTTON);
            break;
        case WM_MBUTTONUP:
            Input::HandleMouseUp(VK_MBUTTON);
            break;
        case WM_XBUTTONDOWN:
            {
                if (const WORD which = HIWORD(wParam); which == XBUTTON1)
                    Input::HandleMouseDown(VK_XBUTTON1);
                else if (which == XBUTTON2)
                    Input::HandleMouseDown(VK_XBUTTON2);
                return TRUE;
            }
        case WM_XBUTTONUP:
            {
                if (const WORD which = HIWORD(wParam); which == XBUTTON1)
                    Input::HandleMouseUp(VK_XBUTTON1);
                else if (which == XBUTTON2)
                    Input::HandleMouseUp(VK_XBUTTON2);
                return TRUE;
            }

        case WM_SETCURSOR:
            if (LOWORD(lParam) == HTCLIENT)
            {
                Input::UpdateCursor();
                return TRUE;
            }
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

    ATOM RegisterGameClass(HINSTANCE hInstance)
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
            .lpszClassName = WINDOW_CLASS_NAME,
            .hIconSm = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_SMALL))
        };
        return RegisterClassExW(&wcex);
    }

    void attachConsole()
    {
        if (!AttachConsole(ATTACH_PARENT_PROCESS))
        {
            return;
        }

        FILE* fp_out = nullptr;
        freopen_s(&fp_out, "CONOUT$", "w", stdout);

        FILE* fp_err = nullptr;
        freopen_s(&fp_err, "CONOUT$", "w", stderr);

        FILE* fp_in = nullptr;
        freopen_s(&fp_in, "CONIN$", "r", stdin);

        std::ios::sync_with_stdio(true);
    }
}

int APIENTRY wWinMain(HINSTANCE hInstance, HINSTANCE, LPWSTR, int nCmdShow)
{
    attachConsole();

    if (!RegisterGameClass(hInstance))
        return FALSE;

    const HWND hWnd = CreateWindowW(WINDOW_CLASS_NAME, WINDOW_TITLE,
                                    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                                    CW_USEDEFAULT, 0, WINDOW_WIDTH, WINDOW_HEIGHT,
                                    nullptr, nullptr, hInstance, nullptr);

    if (!hWnd) return FALSE;
    ShowWindow(hWnd, nCmdShow);
    UpdateWindow(hWnd);

    Game::Initialize(hWnd);
    Game::SetScene<LoadScene>();
    Game::Run();
    Game::Uninitialize();
    return 0;
}
