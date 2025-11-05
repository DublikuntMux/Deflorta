#include "Renderer.hpp"

#include "D2DRenderBackend.hpp"
#include "../Base/Time.hpp"
#include "../Resource/ReanimationLoader.hpp"

#include <algorithm>
#include <format>
#include <numbers>

std::unique_ptr<IRenderBackend> Renderer::backend_;
bool Renderer::showFPS_ = false;
std::vector<DrawItem> Renderer::drawQueue_;
size_t Renderer::submitSeq_ = 0;

DrawItem::DrawItem(const DrawItem& other)
{
    z = other.z;
    seq = other.seq;
    drawType = other.drawType;
    if (drawType == DrawType::Image)
    {
        new(&data.image) ImageData(other.data.image);
    }
    else if (drawType == DrawType::Text)
    {
        new(&data.text) TextData(other.data.text);
    }
    else if (drawType == DrawType::Rectangle)
    {
        new(&data.rectangle) RectangleData(other.data.rectangle);
    }
}

DrawItem::DrawItem(DrawItem&& other) noexcept
{
    z = other.z;
    seq = other.seq;
    drawType = other.drawType;
    if (drawType == DrawType::Image)
    {
        new(&data.image) ImageData(std::move(other.data.image));
    }
    else if (drawType == DrawType::Text)
    {
        new(&data.text) TextData(std::move(other.data.text));
    }
    else if (drawType == DrawType::Rectangle)
    {
        new(&data.rectangle) RectangleData(other.data.rectangle);
    }
}

DrawItem& DrawItem::operator=(const DrawItem& other)
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
    else if (drawType == DrawType::Text)
    {
        new(&data.text) TextData(other.data.text);
    }
    else if (drawType == DrawType::Rectangle)
    {
        new(&data.rectangle) RectangleData(other.data.rectangle);
    }
    return *this;
}

DrawItem& DrawItem::operator=(DrawItem&& other) noexcept
{
    if (this == &other) return *this;
    DestroyActive();
    z = other.z;
    seq = other.seq;
    drawType = other.drawType;
    if (drawType == DrawType::Image)
    {
        new(&data.image) ImageData(std::move(other.data.image));
    }
    else if (drawType == DrawType::Text)
    {
        new(&data.text) TextData(std::move(other.data.text));
    }
    else if (drawType == DrawType::Rectangle)
    {
        new(&data.rectangle) RectangleData(other.data.rectangle);
    }
    return *this;
}

void DrawItem::DestroyActive()
{
    if (drawType == DrawType::Text)
    {
        data.text.~TextData();
    }
    else if (drawType == DrawType::Rectangle)
    {
        data.rectangle.~RectangleData();
    }
    else
    {
        data.image.~ImageData();
    }
}

bool Renderer::Initialize(void* windowHandle)
{
    backend_ = std::make_unique<D2DRenderBackend>();
    return backend_->Initialize(windowHandle);
}

void Renderer::Resize(uint32_t width, uint32_t height)
{
    if (backend_)
        backend_->Resize(width, height);
}

void Renderer::BeginFrame()
{
    if (!backend_) return;

    drawQueue_.clear();
    if (drawQueue_.capacity() < 2048)
        drawQueue_.reserve(2048);
    submitSeq_ = 0;

    backend_->BeginFrame();
    backend_->Clear(Color::Black());
}

void Renderer::Render()
{
    if (!backend_) return;

    FlushDrawQueue();
    DrawFPS();

    backend_->EndFrame();
}

void Renderer::Cleanup()
{
    if (backend_)
        backend_->Shutdown();
    backend_.reset();
}

void Renderer::ToggleFPS()
{
    showFPS_ = !showFPS_;
}

void Renderer::DrawFPS()
{
    if (!showFPS_) return;

    const std::wstring text = std::format(L"FPS: {:.1f}", Time::GetFps());
    const Rect layoutRect(8.0f, 4.0f, 300.0f, 40.0f);

    const auto textFormat = backend_->CreateTextFormat(L"Consolas", 16.0f);
    if (textFormat)
    {
        backend_->DrawTexts(text, layoutRect, textFormat.get(), Color::White());
    }
}

void Renderer::FlushDrawQueue()
{
    if (drawQueue_.empty()) return;

    std::ranges::sort(drawQueue_, [](const DrawItem& a, const DrawItem& b)
    {
        if (a.z != b.z) return a.z < b.z;
        return a.seq < b.seq;
    });

    for (const auto& di : drawQueue_)
    {
        switch (di.drawType)
        {
        case DrawType::Image:
            if (di.data.image.texture)
            {
                backend_->DrawTexture(
                    di.data.image.texture.get(),
                    di.data.image.transform,
                    di.data.image.opacity);
            }
            break;
        case DrawType::Text:
            if (di.data.text.textFormat)
            {
                backend_->DrawTexts(
                    di.data.text.text,
                    di.data.text.rect,
                    di.data.text.textFormat.get(),
                    di.data.text.color);
            }
            break;
        case DrawType::Rectangle:
            backend_->DrawRectangle(
                di.data.rectangle.rect,
                di.data.rectangle.color,
                di.data.rectangle.strokeWidth,
                di.data.rectangle.filled);
            break;
        }
    }

    drawQueue_.clear();
}

void Renderer::EnqueueImage(const std::shared_ptr<ITexture>& texture, const Transform& transform,
                            float opacity, int z)
{
    if (!texture) return;

    const auto size = texture->GetSize();

    const glm::mat3 mat1 = MatrixHelper::Translation(-size / 2.0f);
    const glm::mat3 mat2 = MatrixHelper::Scale(transform.scale);
    const glm::mat3 mat3 = MatrixHelper::Rotation(transform.rotation);
    const glm::mat3 mat4 = MatrixHelper::Translation(transform.position + size / 2.0f);

    const glm::mat3 mat = mat4 * mat3 * mat2 * mat1;

    DrawItem di;
    di.z = z;
    di.seq = submitSeq_++;
    di.data.image.~ImageData();
    new(&di.data.image) DrawItem::ImageData(texture, mat, opacity);
    di.drawType = DrawType::Image;
    drawQueue_.push_back(std::move(di));
}

void Renderer::EnqueueReanim(const std::shared_ptr<ITexture>& texture, const ReanimatorTransform& transform, int z)
{
    if (!texture) return;

    const float kx = transform.skew.x * std::numbers::pi_v<float> / 180.0f;
    const float ky = transform.skew.y * std::numbers::pi_v<float> / 180.0f;

    const float a = std::cos(kx) * transform.scale.x;
    const float b = std::sin(kx) * transform.scale.x;
    const float c = -std::sin(ky) * transform.scale.y;
    const float d = std::cos(ky) * transform.scale.y;

    const glm::mat3 mat = MatrixHelper::CreateMatrix(
        a, b,
        c, d,
        transform.translation.x, transform.translation.y);

    DrawItem di;
    di.z = z;
    di.seq = submitSeq_++;
    di.data.image.~ImageData();
    new(&di.data.image) DrawItem::ImageData(texture, mat, 1.0f);
    di.drawType = DrawType::Image;
    drawQueue_.push_back(std::move(di));
}

void Renderer::EnqueueTextW(const std::wstring& text,
                            const Rect& layoutRect,
                            const std::wstring& fontFamily,
                            float fontSize,
                            const Color& color,
                            int z)
{
    if (text.empty() || !backend_) return;

    const auto textFormat = backend_->CreateTextFormat(fontFamily, fontSize);
    if (!textFormat) return;

    DrawItem di;
    di.z = z;
    di.seq = submitSeq_++;
    di.data.image.~ImageData();
    new(&di.data.text) DrawItem::TextData(text, textFormat, layoutRect, color);
    di.drawType = DrawType::Text;
    drawQueue_.push_back(std::move(di));
}

void Renderer::EnqueueRectangle(const Rect& rect,
                                const Color& color,
                                float strokeWidth,
                                bool filled,
                                int z)
{
    DrawItem di;
    di.z = z;
    di.seq = submitSeq_++;
    di.data.image.~ImageData();
    new(&di.data.rectangle) DrawItem::RectangleData(rect, color, strokeWidth, filled);
    di.drawType = DrawType::Rectangle;
    drawQueue_.push_back(std::move(di));
}

IRenderBackend* Renderer::GetRenderBackend()
{
    return backend_.get();
}
