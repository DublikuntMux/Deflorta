#pragma once

#include "PixelData.hpp"
#include "../Base/Color.hpp"
#include "../Base/Matrix.hpp"
#include "../Base/Rect.hpp"

#include <memory>
#include <string>

class ITexture
{
public:
    virtual ~ITexture() = default;
    virtual glm::vec2 GetSize() const = 0;
    virtual void* GetNativeHandle() const = 0;
};

class ITextFormat
{
public:
    virtual ~ITextFormat() = default;
    virtual void* GetNativeHandle() const = 0;
};

class IRenderBackend
{
public:
    virtual ~IRenderBackend() = default;

    virtual bool Initialize(void* windowHandle) = 0;
    virtual void Shutdown() = 0;

    virtual bool CreateDeviceResources(void* windowHandle) = 0;
    virtual void DiscardDeviceResources() = 0;
    virtual void Resize(uint32_t width, uint32_t height) = 0;

    virtual void BeginFrame() = 0;
    virtual void EndFrame() = 0;
    virtual void Clear(const Color& color) = 0;

    virtual std::shared_ptr<ITexture> CreateTexture(const PixelData& data) = 0;

    virtual std::shared_ptr<ITextFormat> CreateTextFormat(
        const std::wstring& fontFamily,
        float fontSize) = 0;

    virtual void DrawTexture(
        ITexture* texture,
        const glm::mat3& transform,
        float opacity) = 0;

    virtual void DrawTextureRect(
        ITexture* texture,
        const glm::mat3& transform,
        const Rect& sourceRect,
        float opacity) = 0;

    virtual void DrawTexts(
        const std::wstring& text,
        const Rect& layoutRect,
        ITextFormat* textFormat,
        const Color& color,
        float opacity) = 0;

    virtual void DrawRectangle(
        const Rect& rect,
        const Color& color,
        float strokeWidth,
        bool filled = false) = 0;

    virtual void SetTransform(const glm::mat3& transform) = 0;
    virtual glm::mat3 GetTransform() const = 0;

    virtual void Lock() = 0;
    virtual void Unlock() = 0;
};

class RenderBackendLock
{
public:
    explicit RenderBackendLock(IRenderBackend* backend) : backend_(backend)
    {
        if (backend_) backend_->Lock();
    }

    ~RenderBackendLock()
    {
        if (backend_) backend_->Unlock();
    }

    RenderBackendLock(const RenderBackendLock&) = delete;
    RenderBackendLock& operator=(const RenderBackendLock&) = delete;

private:
    IRenderBackend* backend_;
};
