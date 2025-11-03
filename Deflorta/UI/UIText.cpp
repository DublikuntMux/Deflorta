#include "UIText.hpp"

#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

UIText::UIText(std::wstring text, std::wstring font, float size, const D2D1_COLOR_F& color,
               float x, float y, float width, float height)
    : text_(std::move(text)), font_(std::move(font)), size_(size), width_(width), height_(height), color_(color)
{
    x_ = x;
    y_ = y;
}

void UIText::Render()
{
    if (visible_)
    {
        const D2D1_RECT_F rect = D2D1::RectF(x_, y_, x_ + width_, y_ + height_);
        Renderer::EnqueueTextW(text_, rect, font_, size_, color_, static_cast<int>(RenderLayer::UI));
    }
}

std::wstring UIText::GetText() const
{
    return text_;
}

void UIText::SetText(std::wstring text)
{
    text_ = std::move(text);
}

std::wstring UIText::GetFont() const
{
    return font_;
}

void UIText::SetFont(std::wstring font)
{
    font_ = std::move(font);
}

float UIText::GetSize() const
{
    return size_;
}

void UIText::SetSize(float size)
{
    size_ = size;
}

D2D1_COLOR_F UIText::GetColor() const
{
    return color_;
}

void UIText::SetColor(const D2D1_COLOR_F& color)
{
    color_ = color;
}
