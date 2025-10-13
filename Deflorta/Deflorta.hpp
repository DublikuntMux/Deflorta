#pragma once

#include <d2d1_1.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <optional>

using Microsoft::WRL::ComPtr;

class Deflorta final
{
public:
    Deflorta() = default;
    ~Deflorta() = default;

    bool initialize(HWND hwnd);
    void resize(UINT width, UINT height);
    void render();
    void cleanup();

private:
    std::optional<HRESULT> createDeviceResources(HWND hwnd);
    void discardDeviceResources();
    void recreateTargetBitmap();

    HWND hwnd_ = nullptr;

    ComPtr<ID3D11Device> d3dDevice_;
    ComPtr<ID3D11DeviceContext> d3dContext_;
    ComPtr<IDXGISwapChain1> swapChain_;

    ComPtr<ID2D1Factory1> d2dFactory_;
    ComPtr<ID2D1Device> d2dDevice_;
    ComPtr<ID2D1DeviceContext> d2dContext_;
    ComPtr<ID2D1Bitmap1> d2dTargetBitmap_;
    ComPtr<ID2D1SolidColorBrush> brush_;
};