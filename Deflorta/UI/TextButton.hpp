#pragma once

#include <string>

#include "Button.hpp"

class TextButton : public Button
{
public:
    TextButton(
        std::wstring text,
        std::wstring font,
        float size,
        const D2D1_COLOR_F& normalColor,
        const D2D1_COLOR_F& hoverColor,
        float x = 0.0f,
        float y = 0.0f,
        float width = 0.0f,
        float height = 0.0f
    );

    void Render() override;

    [[nodiscard]] std::wstring GetText() const;
    void SetText(std::wstring text);

    [[nodiscard]] std::wstring GetFont() const;
    void SetFont(std::wstring font);

    [[nodiscard]] float GetSize() const;
    void SetSize(float size);

    [[nodiscard]] D2D1_COLOR_F GetNormalColor() const;
    void SetNormalColor(const D2D1_COLOR_F& normal_color);

    [[nodiscard]] D2D1_COLOR_F GetHoverColor() const;
    void SetHoverColor(const D2D1_COLOR_F& hover_color);

private:
    std::wstring text_;
    std::wstring font_;
    float size_;
    D2D1_COLOR_F normalColor_;
    D2D1_COLOR_F hoverColor_;
};
