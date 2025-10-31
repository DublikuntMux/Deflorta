#include "ImageButton.hpp"

#include "../Base/Transform.hpp"
#include "../Render/Renderer.hpp"

ImageButton::ImageButton(ID2D1Bitmap* normalImage, ID2D1Bitmap* hoverImage,
                         float x, float y, float width, float height)
    : normalImage_(normalImage), hoverImage_(hoverImage)
{
    x_ = x;
    y_ = y;
    width_ = width;
    height_ = height;
}

void ImageButton::Render()
{
    if (visible_)
    {
        if (ID2D1Bitmap* currentImage = state_ == State::Hovered || state_ == State::Pressed
                                            ? hoverImage_
                                            : normalImage_)
        {
            Transform transform;
            transform.x = x_;
            transform.y = y_;
            Renderer::EnqueueImage(currentImage, transform, 1.0f, zOrder_);
        }
    }
}

ID2D1Bitmap* ImageButton::GetNormalImage() const
{
    return normalImage_;
}

void ImageButton::SetNormalImage(ID2D1Bitmap* normal_image)
{
    normalImage_ = normal_image;
}

ID2D1Bitmap* ImageButton::GetHoverImage() const
{
    return hoverImage_;
}

void ImageButton::SetHoverImage(ID2D1Bitmap* hover_image)
{
    hoverImage_ = hover_image;
}
