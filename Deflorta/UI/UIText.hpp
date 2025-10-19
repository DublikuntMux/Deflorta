#pragma once

#include <string>

#include "Widget.hpp"

class UIText : public Widget
{
public:
    UIText(
        std::wstring text,
        std::wstring font,
        float size,
        const D2D1_COLOR_F& color,
        const D2D1_RECT_F& rect
    );

    void Render() override;

    [[nodiscard]] std::wstring GetText() const;
    void SetText(std::wstring text);

    [[nodiscard]] std::wstring GetFont() const;
    void SetFont(std::wstring font);

    [[nodiscard]] float GetSize() const;
    void SetSize(float size);

    [[nodiscard]] D2D1_COLOR_F GetColor() const;
    void SetColor(const D2D1_COLOR_F& color);

private:
    std::wstring text_;
    std::wstring font_;
    float size_;
    D2D1_COLOR_F color_;
};
