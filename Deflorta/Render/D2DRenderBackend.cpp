#include "D2DRenderBackend.hpp"

#include <Windows.h>

#include <stdexcept>

D2DRenderBackend::~D2DRenderBackend()
{
    D2DRenderBackend::Shutdown();
}

bool D2DRenderBackend::Initialize(void* windowHandle)
{
    hwnd_ = windowHandle;

    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

    D2D1_FACTORY_OPTIONS factoryOptions;
#ifdef _DEBUG
    factoryOptions.debugLevel = D2D1_DEBUG_LEVEL_INFORMATION;
#endif

    HRESULT hr = D2D1CreateFactory(
        D2D1_FACTORY_TYPE_MULTI_THREADED,
        factoryOptions,
        d2dFactory_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    hr = DWriteCreateFactory(
        DWRITE_FACTORY_TYPE_SHARED,
        __uuidof(IDWriteFactory),
        reinterpret_cast<IUnknown**>(dwFactory_.ReleaseAndGetAddressOf()));
    if (FAILED(hr)) return false;

    return CreateDeviceResources(windowHandle);
}

void D2DRenderBackend::Shutdown()
{
    std::lock_guard lock(mutex_);

    DiscardDeviceResources();
    d2dFactory_.Reset();
    dwFactory_.Reset();
    textFormatCache_.clear();
}

bool D2DRenderBackend::CreateDeviceResources(void* windowHandle)
{
    const auto hwnd = static_cast<HWND>(windowHandle);

    UINT creationFlags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
#ifdef _DEBUG
    creationFlags |= D3D11_CREATE_DEVICE_DEBUG;
#endif

    constexpr D3D_FEATURE_LEVEL fl = D3D_FEATURE_LEVEL_11_0;
    HRESULT hr = D3D11CreateDevice(
        nullptr,
        D3D_DRIVER_TYPE_HARDWARE,
        nullptr,
        creationFlags,
        &fl,
        1,
        D3D11_SDK_VERSION,
        d3dDevice_.ReleaseAndGetAddressOf(),
        nullptr,
        d3dContext_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    Microsoft::WRL::ComPtr<IDXGIDevice> dxgiDevice;
    d3dDevice_.As(&dxgiDevice);

    Microsoft::WRL::ComPtr<IDXGIAdapter> adapter;
    dxgiDevice->GetAdapter(adapter.GetAddressOf());

    Microsoft::WRL::ComPtr<IDXGIFactory2> dxgiFactory;
    adapter->GetParent(IID_PPV_ARGS(&dxgiFactory));

    hr = d2dFactory_->CreateDevice(dxgiDevice.Get(), d2dDevice_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    hr = d2dDevice_->CreateDeviceContext(
        D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
        d2dContext_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    d2dContext_->SetUnitMode(D2D1_UNIT_MODE_PIXELS);

    DXGI_SWAP_CHAIN_DESC1 scd{};
    scd.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    scd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    scd.SampleDesc.Count = 1;
    scd.BufferCount = 3;
    scd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    scd.Scaling = DXGI_SCALING_NONE;
    scd.AlphaMode = DXGI_ALPHA_MODE_IGNORE;

    hr = dxgiFactory->CreateSwapChainForHwnd(
        d3dDevice_.Get(),
        hwnd,
        &scd,
        nullptr,
        nullptr,
        swapChain_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return false;

    RecreateTargetBitmap();

    return true;
}

void D2DRenderBackend::DiscardDeviceResources()
{
    std::lock_guard lock(mutex_);

    brush_.Reset();
    d2dTargetBitmap_.Reset();
    swapChain_.Reset();
    d2dContext_.Reset();
    d2dDevice_.Reset();
    d3dContext_.Reset();
    d3dDevice_.Reset();
}

void D2DRenderBackend::Resize(uint32_t width, uint32_t height)
{
    std::lock_guard lock(mutex_);
    if (!swapChain_) return;

    d2dContext_->SetTarget(nullptr);
    d2dTargetBitmap_.Reset();
    swapChain_->ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
    RecreateTargetBitmap();
}

void D2DRenderBackend::RecreateTargetBitmap()
{
    std::lock_guard lock(mutex_);

    Microsoft::WRL::ComPtr<IDXGISurface> dxgiSurface;
    HRESULT hr = swapChain_->GetBuffer(0, IID_PPV_ARGS(&dxgiSurface));
    if (FAILED(hr)) return;

    const D2D1_BITMAP_PROPERTIES1 props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));

    hr = d2dContext_->CreateBitmapFromDxgiSurface(
        dxgiSurface.Get(),
        &props,
        d2dTargetBitmap_.ReleaseAndGetAddressOf());
    if (FAILED(hr)) return;

    d2dContext_->SetTarget(d2dTargetBitmap_.Get());
    d2dContext_->CreateSolidColorBrush(
        D2D1::ColorF(D2D1::ColorF::White),
        brush_.ReleaseAndGetAddressOf());
}

void D2DRenderBackend::BeginFrame()
{
    std::lock_guard lock(mutex_);

    if (!swapChain_)
    {
        CreateDeviceResources(hwnd_);
        if (!swapChain_) return;
    }

    d2dContext_->BeginDraw();
}

void D2DRenderBackend::EndFrame()
{
    std::lock_guard lock(mutex_);

    const HRESULT hr = d2dContext_->EndDraw();
    if (SUCCEEDED(hr))
    {
        swapChain_->Present(0, 0);
    }
    else if (hr == D2DERR_RECREATE_TARGET)
    {
        DiscardDeviceResources();
    }
}

void D2DRenderBackend::Clear(const Color& color)
{
    std::lock_guard lock(mutex_);
    d2dContext_->Clear(ConvertColor(color));
}

std::shared_ptr<ITexture> D2DRenderBackend::CreateTexture(const PixelData& data)
{
    std::lock_guard lock(mutex_);

    if (!d2dContext_ || data.pixels.empty() || data.width == 0 || data.height == 0)
        return nullptr;

    const D2D1_BITMAP_PROPERTIES props = D2D1::BitmapProperties(
        D2D1::PixelFormat(DXGI_FORMAT_R8G8B8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED));

    Microsoft::WRL::ComPtr<ID2D1Bitmap> bitmap;
    const HRESULT hr = d2dContext_->CreateBitmap(
        D2D1::SizeU(data.width, data.height),
        data.pixels.data(),
        data.pitch ? data.pitch : data.width * 4,
        &props,
        bitmap.GetAddressOf());

    if (FAILED(hr)) return nullptr;

    return std::make_shared<D2DTexture>(bitmap);
}

std::shared_ptr<ITextFormat> D2DRenderBackend::CreateTextFormat(
    const std::wstring& fontFamily,
    float fontSize)
{
    std::lock_guard lock(mutex_);

    const std::wstring key = fontFamily + L"|" + std::to_wstring(fontSize);

    const auto it = textFormatCache_.find(key);
    if (it != textFormatCache_.end())
        return it->second;

    Microsoft::WRL::ComPtr<IDWriteTextFormat> format;
    const HRESULT hr = dwFactory_->CreateTextFormat(
        fontFamily.c_str(),
        nullptr,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        fontSize,
        L"en-us",
        format.GetAddressOf());

    if (FAILED(hr)) return nullptr;

    auto textFormat = std::make_shared<DWriteTextFormat>(format);
    textFormatCache_[key] = textFormat;
    return textFormat;
}

void D2DRenderBackend::DrawTexture(
    ITexture* texture,
    const glm::mat3& transform,
    float opacity)
{
    std::lock_guard lock(mutex_);

    if (!texture) return;

    const auto d2dTexture = dynamic_cast<D2DTexture*>(texture);
    ID2D1Bitmap* bitmap = d2dTexture->GetBitmap();
    if (!bitmap) return;

    const auto size = bitmap->GetSize();

    d2dContext_->SetTransform(ConvertMatrix(transform));
    d2dContext_->DrawBitmap(
        bitmap,
        D2D1::RectF(0, 0, size.width, size.height),
        opacity);
    d2dContext_->SetTransform(D2D1::Matrix3x2F::Identity());
}

void D2DRenderBackend::DrawTexts(
    const std::wstring& text,
    const Rect& layoutRect,
    ITextFormat* textFormat,
    const Color& color)
{
    std::lock_guard lock(mutex_);

    if (text.empty() || !textFormat) return;

    if (!brush_)
    {
        d2dContext_->CreateSolidColorBrush(
            D2D1::ColorF(D2D1::ColorF::White),
            brush_.ReleaseAndGetAddressOf());
        if (!brush_) return;
    }

    const auto dwFormat = dynamic_cast<DWriteTextFormat*>(textFormat);

    brush_->SetColor(ConvertColor(color));

    d2dContext_->DrawTextW(
        text.c_str(),
        static_cast<UINT32>(text.size()),
        dwFormat->GetFormat(),
        ConvertRect(layoutRect),
        brush_.Get());
}

void D2DRenderBackend::DrawRectangle(
    const Rect& rect,
    const Color& color,
    float strokeWidth,
    bool filled)
{
    std::lock_guard lock(mutex_);

    if (!brush_)
    {
        d2dContext_->CreateSolidColorBrush(
            D2D1::ColorF(D2D1::ColorF::White),
            brush_.ReleaseAndGetAddressOf());
        if (!brush_) return;
    }

    brush_->SetColor(ConvertColor(color));

    const D2D1_RECT_F d2dRect = ConvertRect(rect);

    if (filled)
    {
        d2dContext_->FillRectangle(d2dRect, brush_.Get());
    }
    else
    {
        d2dContext_->DrawRectangle(d2dRect, brush_.Get(), strokeWidth);
    }
}

void D2DRenderBackend::SetTransform(const glm::mat3& transform)
{
    std::lock_guard lock(mutex_);
    d2dContext_->SetTransform(ConvertMatrix(transform));
}

glm::mat3 D2DRenderBackend::GetTransform() const
{
    D2D1_MATRIX_3X2_F mat;
    d2dContext_->GetTransform(&mat);

    return MatrixHelper::CreateMatrix(
        mat._11, mat._12,
        mat._21, mat._22,
        mat._31, mat._32);
}

void D2DRenderBackend::Lock()
{
    mutex_.lock();
}

void D2DRenderBackend::Unlock()
{
    mutex_.unlock();
}

D2D1_MATRIX_3X2_F D2DRenderBackend::ConvertMatrix(const glm::mat3& mat)
{
    return D2D1::Matrix3x2F(
        mat[0][0], mat[0][1],
        mat[1][0], mat[1][1],
        mat[2][0], mat[2][1]);
}

D2D1_COLOR_F D2DRenderBackend::ConvertColor(const Color& color)
{
    return D2D1::ColorF(color.value.r, color.value.g, color.value.b, color.value.a);
}

D2D1_RECT_F D2DRenderBackend::ConvertRect(const Rect& rect)
{
    return D2D1::RectF(rect.min.x, rect.min.y, rect.max.x, rect.max.y);
}
