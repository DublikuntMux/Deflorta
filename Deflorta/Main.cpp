#include <memory>

#include "Renderer.hpp"
#include "resource.h"

static std::unique_ptr<Renderer> g_renderer;

static LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam)
{
    switch (message)
    {
    case WM_KEYDOWN:
        if (wParam == VK_F1 && g_renderer)
            g_renderer->toggleFPS();
        break;

    case WM_PAINT:
        {
            PAINTSTRUCT ps;
            BeginPaint(hWnd, &ps);
            if (g_renderer)
                g_renderer->render();
            EndPaint(hWnd, &ps);
        }
        break;

    case WM_SIZE:
        if (g_renderer)
            g_renderer->resize(LOWORD(lParam), HIWORD(lParam));
        break;

    case WM_DESTROY:
        if (g_renderer)
            g_renderer->cleanup();
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

static BOOL InitInstance(HINSTANCE hInstance, int nCmdShow)
{
    const HWND hWnd = CreateWindowW(L"Destolfa", L"Destolfa",
                                    WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, 0, 960, 640,
                                    nullptr, nullptr, hInstance, nullptr);

    if (!hWnd) return FALSE;

    g_renderer = std::make_unique<Renderer>();
    if (!g_renderer->initialize(hWnd))
        return FALSE;

    ShowWindow(hWnd, nCmdShow);
    UpdateWindow(hWnd);
    return TRUE;
}

int APIENTRY wWinMain(HINSTANCE hInstance, HINSTANCE, LPWSTR, int nCmdShow)
{
    MyRegisterClass(hInstance);
    if (!InitInstance(hInstance, nCmdShow))
        return FALSE;

    MSG msg{};
    while (GetMessage(&msg, nullptr, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }

    g_renderer.reset();
    return static_cast<int>(msg.wParam);
}