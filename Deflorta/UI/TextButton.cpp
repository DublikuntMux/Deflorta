#include "TextButton.hpp"

#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

TextButton::TextButton(std::wstring text,
                       std::wstring font,
                       float size,
                       const Color& normalColor,
                       const Color& hoverColor,
                       glm::vec2 position,
                       glm::vec2 dimensions)
    : text_(std::move(text)), font_(std::move(font)), fontSize_(size), normalColor_(normalColor),
      hoverColor_(hoverColor)
{
    position_ = position;
    dimensions_ = dimensions;
}

void TextButton::Render()
{
    if (visible_)
    {
        const Color currentColor = state_ == State::Hovered || state_ == State::Pressed
                                       ? hoverColor_
                                       : normalColor_;
        const auto rect = Rect(position_.x, position_.y, position_.x + dimensions_.x, position_.y + dimensions_.y);
        Renderer::EnqueueTextW(text_, rect, font_, fontSize_, currentColor, static_cast<int>(RenderLayer::UI));
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

float TextButton::GetFontSize() const
{
    return fontSize_;
}

void TextButton::SetFontSize(float size)
{
    fontSize_ = size;
}

Color TextButton::GetNormalColor() const
{
    return normalColor_;
}

void TextButton::SetNormalColor(const Color& normal_color)
{
    normalColor_ = normal_color;
}

Color TextButton::GetHoverColor() const
{
    return hoverColor_;
}

void TextButton::SetHoverColor(const Color& hover_color)
{
    hoverColor_ = hover_color;
}
