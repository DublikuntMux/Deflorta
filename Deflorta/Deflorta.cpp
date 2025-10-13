#include "Deflorta.hpp"
#include "resource.h"

#include <d2d1_1.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <memory>
#include <optional>

#pragma comment(lib, "d2d1.lib")
#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")

bool Deflorta::initialize(HWND hwnd)
{
    hwnd_ = hwnd;

    D2D1_FACTORY_OPTIONS factoryOptions;
#ifdef _DEBUG
    factoryOptions.debugLevel = D2D1_DEBUG_LEVEL_INFORMATION;
#endif
    const HRESULT hr = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factoryOptions, d2dFactory_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    return !createDeviceResources(hwnd).has_value();
}

std::optional<HRESULT> Deflorta::createDeviceResources(HWND hwnd)
{
    UINT creationFlags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
#ifdef _DEBUG
    creationFlags |= D3D11_CREATE_DEVICE_DEBUG;
#endif

    D3D_FEATURE_LEVEL featureLevels[] = { D3D_FEATURE_LEVEL_11_0 };
    HRESULT hr = D3D11CreateDevice(
        nullptr,
        D3D_DRIVER_TYPE_HARDWARE,
        nullptr,
        creationFlags,
        featureLevels,
        ARRAYSIZE(featureLevels),
        D3D11_SDK_VERSION,
        d3dDevice_.ReleaseAndGetAddressOf(),
        nullptr,
        d3dContext_.ReleaseAndGetAddressOf()
    );
    if (FAILED(hr)) return hr;

    ComPtr<IDXGIDevice> dxgiDevice;
    d3dDevice_.As(&dxgiDevice);

    ComPtr<IDXGIAdapter> adapter;
    dxgiDevice->GetAdapter(adapter.GetAddressOf());

    ComPtr<IDXGIFactory2> dxgiFactory;
    adapter->GetParent(IID_PPV_ARGS(&dxgiFactory));

    hr = d2dFactory_->CreateDevice(dxgiDevice.Get(), d2dDevice_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    hr = d2dDevice_->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, d2dContext_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    DXGI_SWAP_CHAIN_DESC1 swapDesc{};
    swapDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    swapDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    swapDesc.SampleDesc.Count = 1;
    swapDesc.BufferCount = 3; 
    swapDesc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    swapDesc.Width = 0;  
    swapDesc.Height = 0; 
    swapDesc.Scaling = DXGI_SCALING_STRETCH;
    swapDesc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;

    hr = dxgiFactory->CreateSwapChainForHwnd(
        d3dDevice_.Get(),
        hwnd,
        &swapDesc,
        nullptr,
        nullptr,
        swapChain_.ReleaseAndGetAddressOf()
    );
    if (FAILED(hr)) return hr;

    recreateTargetBitmap();

    return std::nullopt;
}

void Deflorta::recreateTargetBitmap()
{
    ComPtr<IDXGISurface> dxgiBackBuffer;
    HRESULT hr = swapChain_->GetBuffer(0, IID_PPV_ARGS(&dxgiBackBuffer));
    if (FAILED(hr)) return;

    constexpr float dpi = 96.0f;
    const D2D1_BITMAP_PROPERTIES1 bitmapProps = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE),
        dpi,
        dpi
    );

    hr = d2dContext_->CreateBitmapFromDxgiSurface(dxgiBackBuffer.Get(), &bitmapProps, d2dTargetBitmap_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return;

    d2dContext_->SetTarget(d2dTargetBitmap_.Get());
    d2dContext_->CreateSolidColorBrush(D2D1::ColorF(D2D1::ColorF::CornflowerBlue), brush_.ReleaseAndGetAddressOf());
}

void Deflorta::resize(UINT width, UINT height)
{
    if (!swapChain_) return;

    d2dContext_->SetTarget(nullptr);
    d2dTargetBitmap_.Reset();
    const HRESULT hr = swapChain_->ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
    if (FAILED(hr)) return;

    recreateTargetBitmap();
}

void Deflorta::render()
{
    if (!swapChain_)
    {
        if (createDeviceResources(hwnd_).has_value())
            return;
    }

    d2dContext_->BeginDraw();
    d2dContext_->Clear(D2D1::ColorF(D2D1::ColorF::Black));

    const auto [width, height] = d2dContext_->GetSize();

    constexpr float margin = 50.f;
    const D2D1_RECT_F rect{ margin, margin, width - margin, height - margin };
    d2dContext_->FillRectangle(rect, brush_.Get());
    d2dContext_->DrawRectangle(rect, brush_.Get(), 2.0f);

    const D2D1_ELLIPSE ellipse{
        D2D1::Point2F(width / 2.f, height / 2.f),
        80.f, 80.f
    };
    d2dContext_->DrawEllipse(ellipse, brush_.Get(), 4.0f);

    if (const HRESULT hr = d2dContext_->EndDraw(); SUCCEEDED(hr))
    {
        swapChain_->Present(1, 0);
    }
    else if (hr == D2DERR_RECREATE_TARGET)
    {
        discardDeviceResources();
    }
}

void Deflorta::discardDeviceResources()
{
    brush_.Reset();
    d2dTargetBitmap_.Reset();
    swapChain_.Reset();
    d2dContext_.Reset();
    d2dDevice_.Reset();
    d3dContext_.Reset();
    d3dDevice_.Reset();
}

void Deflorta::cleanup()
{
    discardDeviceResources();
    d2dFactory_.Reset();
}

static std::unique_ptr<Deflorta> g_renderer;

static LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam)
{
    switch (message)
    {
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
        {
            const UINT width = LOWORD(lParam);
            const UINT height = HIWORD(lParam);
            if (g_renderer)
                g_renderer->resize(width, height);
        }
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
        .lpszClassName = L"Deflorta",
        .hIconSm = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_SMALL))
    };
    return RegisterClassExW(&wcex);
}

static BOOL InitInstance(HINSTANCE hInstance, int nCmdShow)
{
    const HWND hWnd = CreateWindowW(L"Deflorta", L"Deflorta",
                              WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, 0, 1280, 720,
                              nullptr, nullptr, hInstance, nullptr);

    if (!hWnd) return FALSE;

    g_renderer = std::make_unique<Deflorta>();
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
