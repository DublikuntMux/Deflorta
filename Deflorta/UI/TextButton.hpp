#pragma once

#include "Button.hpp"
#include "../Base/Color.hpp"

#include <string>

class TextButton : public Button
{
public:
    TextButton(
        std::wstring text,
        std::wstring font,
        float size,
        const Color& normalColor,
        const Color& hoverColor,
        glm::vec2 position,
        glm::vec2 dimensions = {0.0f, 0.0f}
    );

    void Render() override;

    [[nodiscard]] std::wstring GetText() const;
    void SetText(std::wstring text);

    [[nodiscard]] std::wstring GetFont() const;
    void SetFont(std::wstring font);

    [[nodiscard]] float GetFontSize() const;
    void SetFontSize(float size);

    [[nodiscard]] Color GetNormalColor() const;
    void SetNormalColor(const Color& normal_color);

    [[nodiscard]] Color GetHoverColor() const;
    void SetHoverColor(const Color& hover_color);

private:
    std::wstring text_;
    std::wstring font_;
    float fontSize_;
    Color normalColor_;
    Color hoverColor_;
};
