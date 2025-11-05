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
        float fontSize,
        const Color& color,
        glm::vec2 position,
        glm::vec2 dimensions
    );

    void Render() override;

    [[nodiscard]] std::wstring GetText() const;
    void SetText(std::wstring text);

    [[nodiscard]] std::wstring GetFont() const;
    void SetFont(std::wstring font);

    [[nodiscard]] float GetFontSize() const;
    void SetFontSize(float size);

    [[nodiscard]] Color GetColor() const;
    void SetColor(const Color& color);

private:
    std::wstring text_;
    std::wstring font_;
    float fontSize_;
    glm::vec2 dimensions_;
    Color color_;
};
