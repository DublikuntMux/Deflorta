#pragma once

#include "Widget.hpp"
#include "../Base/Color.hpp"

#include <string>

class UIText : public Widget
{
public:
    UIText(
        std::wstring text,
        std::wstring font,
        float size,
        const Color& color,
        float x,
        float y,
        float width,
        float height
    );

    void Render() override;

    [[nodiscard]] std::wstring GetText() const;
    void SetText(std::wstring text);

    [[nodiscard]] std::wstring GetFont() const;
    void SetFont(std::wstring font);

    [[nodiscard]] float GetSize() const;
    void SetSize(float size);

    [[nodiscard]] Color GetColor() const;
    void SetColor(const Color& color);

private:
    std::wstring text_;
    std::wstring font_;
    float size_;
    float width_;
    float height_;
    Color color_;
};
