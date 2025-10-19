#pragma once

#include <optional>
#include <mutex>
#include <string>
#include <vector>
#include <unordered_map>

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include "../Base/Transform.hpp"

struct ReanimatorTransform;

enum class DrawType : std::uint8_t
{
    Image,
    Text
};

struct DrawItem
{
    struct ImageData
    {
        ID2D1Bitmap* bmp = nullptr;
        D2D1_MATRIX_3X2_F t{};
        float opacity = 1.0f;
        ImageData() = default;

        ImageData(ID2D1Bitmap* b, const D2D1_MATRIX_3X2_F& m, float o) : bmp(b), t(m), opacity(o)
        {
        }
    };

    struct TextData
    {
        std::wstring text;
        std::wstring font;
        D2D1_RECT_F rect{};
        float fontSize = 0.0f;
        D2D1_COLOR_F color{};
        TextData() = default;

        TextData(std::wstring tx, std::wstring ft, const D2D1_RECT_F& r, float fs, const D2D1_COLOR_F& c)
            : text(std::move(tx)), font(std::move(ft)), rect(r), fontSize(fs), color(c)
        {
        }
    };

    union ItemData
    {
        ImageData image;
        TextData text;

        ItemData()
        {
        }

        ~ItemData()
        {
        }
    };

    int z = 0;
    size_t seq = 0;
    DrawType drawType = DrawType::Image;
    ItemData data;

    DrawItem()
    {
        new(&data.image) ImageData();
    }

    ~DrawItem()
    {
        DestroyActive();
    }

    DrawItem(const DrawItem& other)
    {
        z = other.z;
        seq = other.seq;
        drawType = other.drawType;
        if (drawType == DrawType::Image)
        {
            new(&data.image) ImageData(other.data.image);
        }
        else
        {
            new(&data.text) TextData(other.data.text);
        }
    }

    DrawItem(DrawItem&& other) noexcept
    {
        z = other.z;
        seq = other.seq;
        drawType = other.drawType;
        if (drawType == DrawType::Image)
        {
            new(&data.image) ImageData(other.data.image);
        }
        else
        {
            new(&data.text) TextData(std::move(other.data.text));
        }
    }

    DrawItem& operator=(const DrawItem& other)
    {
        if (this == &other) return *this;
        DestroyActive();
        z = other.z;
        seq = other.seq;
        drawType = other.drawType;
        if (drawType == DrawType::Image)
        {
            new(&data.image) ImageData(other.data.image);
        }
        else
        {
            new(&data.text) TextData(other.data.text);
        }
        return *this;
    }

    DrawItem& operator=(DrawItem&& other) noexcept
    {
        if (this == &other) return *this;
        DestroyActive();
        z = other.z;
        seq = other.seq;
        drawType = other.drawType;
        if (drawType == DrawType::Image)
        {
            new(&data.image) ImageData(other.data.image);
        }
        else
        {
            new(&data.text) TextData(std::move(other.data.text));
        }
        return *this;
    }

private:
    void DestroyActive()
    {
        if (drawType == DrawType::Text)
        {
            data.text.~TextData();
        }
        else
        {
            data.image.~ImageData();
        }
    }
};

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

    static void EnqueueImage(ID2D1Bitmap* bitmap, const Transform& transform, float opacity = 1.0f, int z = 0);
    static void EnqueueReanim(ID2D1Bitmap* bitmap, const ReanimatorTransform& transform, int z);
    static void EnqueueTextW(const std::wstring& text,
                             const D2D1_RECT_F& layoutRect,
                             const std::wstring& fontFamily,
                             float fontSize,
                             const D2D1_COLOR_F& color,
                             int z);

private:
    static std::optional<HRESULT> CreateDeviceResources(HWND hwnd);
    static void DiscardDeviceResources();
    static void RecreateTargetBitmap();
    static void DrawFPS();
    static void FlushDrawQueue();

    static void DrawImage(ID2D1Bitmap* bitmap, const D2D1_MATRIX_3X2_F& transform, float opacity);
    static void DrawTextW(const std::wstring& text,
                          const D2D1_RECT_F& layoutRect,
                          const std::wstring& fontFamily,
                          float fontSize,
                          const D2D1_COLOR_F& color);

    static IDWriteTextFormat* GetOrCreateTextFormat(const std::wstring& fontFamily, float fontSize);

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

    static std::vector<DrawItem> drawQueue_;
    static size_t submitSeq_;

    static std::unordered_map<std::wstring, Microsoft::WRL::ComPtr<IDWriteTextFormat>> textFormatCache_;
};
