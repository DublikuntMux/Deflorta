#pragma once

#include "Button.hpp"

class ImageButton : public Button
{
public:
    ImageButton(
        ID2D1Bitmap* normalImage,
        ID2D1Bitmap* hoverImage,
        float x = 0.0f,
        float y = 0.0f,
        float width = 0.0f,
        float height = 0.0f
    );

    void Render() override;

    [[nodiscard]] ID2D1Bitmap* GetNormalImage() const;
    void SetNormalImage(ID2D1Bitmap* normal_image);

    [[nodiscard]] ID2D1Bitmap* GetHoverImage() const;
    void SetHoverImage(ID2D1Bitmap* hover_image);

private:
    ID2D1Bitmap* normalImage_;
    ID2D1Bitmap* hoverImage_;
};
