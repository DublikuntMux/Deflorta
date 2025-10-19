#pragma once

#include "Button.hpp"

class ImageButton : public Button
{
public:
    ImageButton(
        ID2D1Bitmap* normalImage,
        ID2D1Bitmap* hoverImage,
        const D2D1_RECT_F& rect
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
