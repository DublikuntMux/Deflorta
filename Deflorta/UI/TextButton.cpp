#include "TextButton.hpp"

#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

TextButton::TextButton(std::wstring text, std::wstring font, float size, const D2D1_COLOR_F& normalColor,
                       const D2D1_COLOR_F& hoverColor, float x, float y, float width, float height)
    : text_(std::move(text)), font_(std::move(font)), size_(size), normalColor_(normalColor), hoverColor_(hoverColor)
{
    x_ = x;
    y_ = y;
    width_ = width;
    height_ = height;
}

void TextButton::Render()
{
    if (visible_)
    {
        const D2D1_COLOR_F currentColor = state_ == State::Hovered || state_ == State::Pressed
                                              ? hoverColor_
                                              : normalColor_;
        const D2D1_RECT_F rect = D2D1::RectF(x_, y_, x_ + width_, y_ + height_);
        Renderer::EnqueueTextW(text_, rect, font_, size_, currentColor, static_cast<int>(RenderLayer::UI));
    }
}

std::wstring TextButton::GetText() const
{
    return text_;
}

void TextButton::SetText(std::wstring text)
{
    text_ = std::move(text);
}

std::wstring TextButton::GetFont() const
{
    return font_;
}

void TextButton::SetFont(std::wstring font)
{
    font_ = std::move(font);
}

float TextButton::GetSize() const
{
    return size_;
}

void TextButton::SetSize(float size)
{
    size_ = size;
}

D2D1_COLOR_F TextButton::GetNormalColor() const
{
    return normalColor_;
}

void TextButton::SetNormalColor(const D2D1_COLOR_F& normal_color)
{
    normalColor_ = normal_color;
}

D2D1_COLOR_F TextButton::GetHoverColor() const
{
    return hoverColor_;
}

void TextButton::SetHoverColor(const D2D1_COLOR_F& hover_color)
{
    hoverColor_ = hover_color;
}
