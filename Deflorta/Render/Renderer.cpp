#include "Renderer.hpp"

#include <string>
#include <format>
#include <optional>

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include "../Base/Time.hpp"
#include "../Base/Transform.hpp"

HWND Renderer::hwnd_ = nullptr;

Microsoft::WRL::ComPtr<ID3D11Device> Renderer::d3dDevice_;
Microsoft::WRL::ComPtr<ID3D11DeviceContext> Renderer::d3dContext_;
Microsoft::WRL::ComPtr<IDXGISwapChain1> Renderer::swapChain_;

Microsoft::WRL::ComPtr<ID2D1Factory1> Renderer::d2dFactory_;
Microsoft::WRL::ComPtr<ID2D1Device> Renderer::d2dDevice_;
Microsoft::WRL::ComPtr<ID2D1DeviceContext> Renderer::d2dContext_;
Microsoft::WRL::ComPtr<ID2D1Bitmap1> Renderer::d2dTargetBitmap_;
Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> Renderer::brush_;
Microsoft::WRL::ComPtr<IDWriteFactory> Renderer::dwFactory_;
Microsoft::WRL::ComPtr<IDWriteTextFormat> Renderer::textFormat_;

bool Renderer::showFPS_ = false;

bool Renderer::initialize(HWND hwnd)
{
    hwnd_ = hwnd;

    D2D1_FACTORY_OPTIONS factoryOptions;
#ifdef _DEBUG
    factoryOptions.debugLevel = D2D1_DEBUG_LEVEL_INFORMATION;
#endif
    HRESULT hr = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factoryOptions,
                                   d2dFactory_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    hr = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                             reinterpret_cast<IUnknown**>(dwFactory_.ReleaseAndGetAddressOf()));
    if (FAILED(hr)) return false;

    hr = dwFactory_->CreateTextFormat(
        L"Consolas", nullptr, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL, 16.0f, L"en-us", textFormat_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    return !createDeviceResources(hwnd).has_value();
}

std::optional<HRESULT> Renderer::createDeviceResources(HWND hwnd)
{
    UINT creationFlags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
#ifdef _DEBUG
    creationFlags |= D3D11_CREATE_DEVICE_DEBUG;
#endif

    constexpr D3D_FEATURE_LEVEL fl = D3D_FEATURE_LEVEL_11_0;
    HRESULT hr = D3D11CreateDevice(
        nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, creationFlags,
        &fl, 1, D3D11_SDK_VERSION, d3dDevice_.ReleaseAndGetAddressOf(),
        nullptr, d3dContext_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    Microsoft::WRL::ComPtr<IDXGIDevice> dxgiDevice;
    d3dDevice_.As(&dxgiDevice);

    Microsoft::WRL::ComPtr<IDXGIAdapter> adapter;
    dxgiDevice->GetAdapter(adapter.GetAddressOf());

    Microsoft::WRL::ComPtr<IDXGIFactory2> dxgiFactory;
    adapter->GetParent(IID_PPV_ARGS(&dxgiFactory));

    hr = d2dFactory_->CreateDevice(dxgiDevice.Get(), d2dDevice_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    hr = d2dDevice_->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, d2dContext_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    DXGI_SWAP_CHAIN_DESC1 scd{};
    scd.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    scd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    scd.SampleDesc.Count = 1;
    scd.BufferCount = 3;
    scd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    scd.Scaling = DXGI_SCALING_STRETCH;
    scd.AlphaMode = DXGI_ALPHA_MODE_IGNORE;

    hr = dxgiFactory->CreateSwapChainForHwnd(
        d3dDevice_.Get(), hwnd, &scd, nullptr, nullptr, swapChain_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return hr;

    recreateTargetBitmap();

    return std::nullopt;
}

void Renderer::recreateTargetBitmap()
{
    Microsoft::WRL::ComPtr<IDXGISurface> dxgiSurface;
    HRESULT hr = swapChain_->GetBuffer(0, IID_PPV_ARGS(&dxgiSurface));
    if (FAILED(hr)) return;

    constexpr float dpi = 96.f;
    const D2D1_BITMAP_PROPERTIES1 props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE),
        dpi, dpi);

    hr = d2dContext_->CreateBitmapFromDxgiSurface(dxgiSurface.Get(), &props, d2dTargetBitmap_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return;

    d2dContext_->SetTarget(d2dTargetBitmap_.Get());
    d2dContext_->CreateSolidColorBrush(D2D1::ColorF(D2D1::ColorF::CornflowerBlue), brush_.ReleaseAndGetAddressOf());
}

void Renderer::resize(UINT width, UINT height)
{
    if (!swapChain_) return;

    d2dContext_->SetTarget(nullptr);
    d2dTargetBitmap_.Reset();
    swapChain_->ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
    recreateTargetBitmap();
}

void Renderer::drawFPS()
{
    if (!showFPS_) return;

    const std::wstring text = std::format(L"FPS: {:.1f}", Time::fps());
    const D2D1_RECT_F layoutRect = D2D1::RectF(8.0f, 4.0f, 300.0f, 40.0f);
    Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> textBrush;
    d2dContext_->CreateSolidColorBrush(D2D1::ColorF(D2D1::ColorF::White), textBrush.ReleaseAndGetAddressOf());

    d2dContext_->DrawTextW(
        text.c_str(),
        static_cast<UINT32>(text.size()),
        textFormat_.Get(),
        layoutRect,
        textBrush.Get());
}

void Renderer::beginFrame()
{
    if (!swapChain_)
    {
        if (createDeviceResources(hwnd_).has_value())
            return;
    }

    d2dContext_->BeginDraw();
    d2dContext_->Clear(D2D1::ColorF(D2D1::ColorF::Black));
}

void Renderer::render()
{
    drawFPS();

    const HRESULT hr = d2dContext_->EndDraw();
    if (SUCCEEDED(hr))
        swapChain_->Present(1, 0);
    else if (hr == D2DERR_RECREATE_TARGET)
        discardDeviceResources();
}

void Renderer::toggleFPS() { showFPS_ = !showFPS_; }

ID2D1DeviceContext* Renderer::getD2DContext()
{
    return d2dContext_.Get();
}

void Renderer::discardDeviceResources()
{
    brush_.Reset();
    d2dTargetBitmap_.Reset();
    swapChain_.Reset();
    d2dContext_.Reset();
    d2dDevice_.Reset();
    d3dContext_.Reset();
    d3dDevice_.Reset();
}

void Renderer::cleanup()
{
    discardDeviceResources();
    d2dFactory_.Reset();
    dwFactory_.Reset();
    textFormat_.Reset();
}

void Renderer::drawImage(ID2D1Bitmap* bitmap, const Transform& transform)
{
    if (!bitmap || !d2dContext_) return;

    const D2D1_SIZE_F size = bitmap->GetSize();
    const D2D1_MATRIX_3X2_F mat =
        D2D1::Matrix3x2F::Translation(-size.width / 2.0f, -size.height / 2.0f) *
        D2D1::Matrix3x2F::Scale(transform.scaleX, transform.scaleY, D2D1::Point2F(0, 0)) *
        D2D1::Matrix3x2F::Rotation(transform.rotation, D2D1::Point2F(0, 0)) *
        D2D1::Matrix3x2F::Translation(transform.x + size.width / 2.0f, transform.y + size.height / 2.0f);

    d2dContext_->SetTransform(mat);
    d2dContext_->DrawBitmap(bitmap, D2D1::RectF(0, 0, size.width, size.height));
    d2dContext_->SetTransform(D2D1::Matrix3x2F::Identity());
}
