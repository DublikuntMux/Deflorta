#include "UIText.hpp"

#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

UIText::UIText(std::wstring text,
               std::wstring font,
               float fontSize,
               const Color& color,
               glm::vec2 position,
               glm::vec2 dimensions)
    : text_(std::move(text)), font_(std::move(font)), fontSize_(fontSize), dimensions_(dimensions), color_(color)
{
    position_ = position;
}

void UIText::Render()
{
    if (visible_)
    {
        const auto rect = Rect(position_.x, position_.y, position_.x + dimensions_.x, position_.y + dimensions_.y);
        Renderer::EnqueueTextW(text_, rect, font_, fontSize_, color_, static_cast<int>(RenderLayer::UI));
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

float UIText::GetFontSize() const
{
    return fontSize_;
}

void UIText::SetFontSize(float size)
{
    fontSize_ = size;
}

Color UIText::GetColor() const
{
    return color_;
}

void UIText::SetColor(const Color& color)
{
    color_ = color;
}
