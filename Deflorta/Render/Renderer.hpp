#pragma once

#include <optional>

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include "../Base/Transform.hpp"

class Renderer final
{
public:
    static bool initialize(HWND hwnd);
    static void resize(UINT width, UINT height);
    static void beginFrame();
    static void render();
    static void cleanup();
    static void toggleFPS();
    static ID2D1DeviceContext* getD2DContext();

    static void drawImage(ID2D1Bitmap* bitmap, const Transform& transform, float opacity = 1.0f);

private:
    static std::optional<HRESULT> createDeviceResources(HWND hwnd);
    static void discardDeviceResources();
    static void recreateTargetBitmap();
    static void drawFPS();

    static HWND hwnd_;
    static bool showFPS_;

    static Microsoft::WRL::ComPtr<ID3D11Device> d3dDevice_;
    static Microsoft::WRL::ComPtr<ID3D11DeviceContext> d3dContext_;
    static Microsoft::WRL::ComPtr<IDXGISwapChain1> swapChain_;

    static Microsoft::WRL::ComPtr<ID2D1Factory1> d2dFactory_;
    static Microsoft::WRL::ComPtr<ID2D1Device> d2dDevice_;
    static Microsoft::WRL::ComPtr<ID2D1DeviceContext> d2dContext_;
    static Microsoft::WRL::ComPtr<ID2D1Bitmap1> d2dTargetBitmap_;
    static Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> brush_;
    static Microsoft::WRL::ComPtr<IDWriteFactory> dwFactory_;
    static Microsoft::WRL::ComPtr<IDWriteTextFormat> textFormat_;
};
