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
        const Color& normalColor,
        const Color& hoverColor,
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

    [[nodiscard]] Color GetNormalColor() const;
    void SetNormalColor(const Color& normal_color);

    [[nodiscard]] Color GetHoverColor() const;
    void SetHoverColor(const Color& hover_color);

private:
    std::wstring text_;
    std::wstring font_;
    float size_;
    Color normalColor_;
    Color hoverColor_;
};
