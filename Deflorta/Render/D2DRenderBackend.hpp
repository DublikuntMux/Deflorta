#pragma once

#include "IRenderBackend.hpp"

#include <d2d1_1.h>
#include <dwrite.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <unordered_map>
#include <mutex>

class D2DTexture : public ITexture
{
public:
    explicit D2DTexture(const Microsoft::WRL::ComPtr<ID2D1Bitmap>& bitmap)
        : bitmap_(bitmap)
    {
    }

    glm::vec2 GetSize() const override
    {
        if (!bitmap_) return {0, 0};
        const auto [width, height] = bitmap_->GetSize();
        return {width, height};
    }

    void* GetNativeHandle() const override
    {
        return bitmap_.Get();
    }

    ID2D1Bitmap* GetBitmap() const { return bitmap_.Get(); }

private:
    Microsoft::WRL::ComPtr<ID2D1Bitmap> bitmap_;
};

class DWriteTextFormat : public ITextFormat
{
public:
    explicit DWriteTextFormat(const Microsoft::WRL::ComPtr<IDWriteTextFormat>& format)
        : format_(format)
    {
    }

    void* GetNativeHandle() const override
    {
        return format_.Get();
    }

    IDWriteTextFormat* GetFormat() const { return format_.Get(); }

private:
    Microsoft::WRL::ComPtr<IDWriteTextFormat> format_;
};

class D2DRenderBackend : public IRenderBackend
{
public:
    D2DRenderBackend() = default;
    ~D2DRenderBackend() override;

    bool Initialize(void* windowHandle) override;
    void Shutdown() override;

    bool CreateDeviceResources(void* windowHandle) override;
    void DiscardDeviceResources() override;
    void Resize(uint32_t width, uint32_t height) override;

    void BeginFrame() override;
    void EndFrame() override;
    void Clear(const Color& color) override;

    std::shared_ptr<ITexture> CreateTexture(const PixelData& data) override;
    std::shared_ptr<ITextFormat> CreateTextFormat(
        const std::wstring& fontFamily,
        float fontSize) override;

    void DrawTexture(
        ITexture* texture,
        const glm::mat3& transform,
        float opacity = 1.0f) override;

    void DrawTexts(
        const std::wstring& text,
        const Rect& layoutRect,
        ITextFormat* textFormat,
        const Color& color) override;

    void DrawRectangle(
        const Rect& rect,
        const Color& color,
        float strokeWidth = 1.0f,
        bool filled = false) override;

    void SetTransform(const glm::mat3& transform) override;
    glm::mat3 GetTransform() const override;

    void Lock() override;
    void Unlock() override;

private:
    void RecreateTargetBitmap();
    D2D1_MATRIX_3X2_F ConvertMatrix(const glm::mat3& mat) const;
    D2D1_COLOR_F ConvertColor(const Color& color) const;
    D2D1_RECT_F ConvertRect(const Rect& rect) const;

    void* hwnd_ = nullptr;
    std::recursive_mutex mutex_;

    Microsoft::WRL::ComPtr<ID3D11Device> d3dDevice_;
    Microsoft::WRL::ComPtr<ID3D11DeviceContext> d3dContext_;
    Microsoft::WRL::ComPtr<IDXGISwapChain1> swapChain_;

    Microsoft::WRL::ComPtr<ID2D1Factory1> d2dFactory_;
    Microsoft::WRL::ComPtr<ID2D1Device> d2dDevice_;
    Microsoft::WRL::ComPtr<ID2D1DeviceContext> d2dContext_;
    Microsoft::WRL::ComPtr<ID2D1Bitmap1> d2dTargetBitmap_;
    Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> brush_;
    Microsoft::WRL::ComPtr<IDWriteFactory> dwFactory_;

    std::unordered_map<std::wstring, std::shared_ptr<ITextFormat>> textFormatCache_;
};
