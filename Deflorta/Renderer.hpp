#pragma once

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <chrono>
#include <optional>

using Microsoft::WRL::ComPtr;

class Renderer final
{
public:
    bool initialize(HWND hwnd);
    void resize(UINT width, UINT height);
    void beginFrame();
    void render();
    void cleanup();
    void toggleFPS();

private:
    std::optional<HRESULT> createDeviceResources(HWND hwnd);
    void discardDeviceResources();
    void recreateTargetBitmap();
    void drawFPS();
    void updateFPS();

    HWND hwnd_ = nullptr;

    ComPtr<ID3D11Device> d3dDevice_;
    ComPtr<ID3D11DeviceContext> d3dContext_;
    ComPtr<IDXGISwapChain1> swapChain_;

    ComPtr<ID2D1Factory1> d2dFactory_;
    ComPtr<ID2D1Device> d2dDevice_;
    ComPtr<ID2D1DeviceContext> d2dContext_;
    ComPtr<ID2D1Bitmap1> d2dTargetBitmap_;
    ComPtr<ID2D1SolidColorBrush> brush_;
    ComPtr<IDWriteFactory> dwFactory_;
    ComPtr<IDWriteTextFormat> textFormat_;

    bool showFPS_ = false;
    double fps_ = 0.0;
    uint64_t frameCount_ = 0;
    std::chrono::steady_clock::time_point lastTime_ = std::chrono::steady_clock::now();
};
