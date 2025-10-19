#include "UIText.hpp"

#include "../Render/Renderer.hpp"

UIText::UIText(std::wstring text, std::wstring font, float size, const D2D1_COLOR_F& color,
               const D2D1_RECT_F& rect) : text_(std::move(text)), font_(std::move(font)), size_(size), color_(color)
{
    rect_ = rect;
}

void UIText::Render()
{
    if (visible_)
    {
        Renderer::EnqueueTextW(text_, rect_, font_, size_, color_, zOrder_);
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
