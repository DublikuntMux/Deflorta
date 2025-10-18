#pragma once

#include <optional>
#include <mutex>

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include "../Base/Transform.hpp"

struct ReanimatorTransform;

class Renderer final
{
public:
    class D2DGuard
    {
    public:
        D2DGuard();
        ~D2DGuard();
    };

    static bool Initialize(HWND hwnd);
    static void Resize(UINT width, UINT height);
    static void BeginFrame();
    static void Render();
    static void Cleanup();
    static void ToggleFPS();
    static ID2D1DeviceContext* GetD2DContext();

    static void DrawImage(ID2D1Bitmap* bitmap, const Transform& transform, float opacity = 1.0f);
    static void DrawReanim(ID2D1Bitmap* bitmap, const ReanimatorTransform& transform, float opacity = 1.0f);

    static void DrawTextW(const std::wstring& text,
                          const D2D1_RECT_F& layoutRect,
                          const std::wstring& fontFamily,
                          float fontSize,
                          const D2D1_COLOR_F& color);

private:
    static std::optional<HRESULT> CreateDeviceResources(HWND hwnd);
    static void DiscardDeviceResources();
    static void RecreateTargetBitmap();
    static void DrawFPS();

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

    static std::recursive_mutex d2dMutex_;
};
