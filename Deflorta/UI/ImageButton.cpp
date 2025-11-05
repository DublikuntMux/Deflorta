#include "ImageButton.hpp"

#include "../Base/Transform.hpp"
#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

ImageButton::ImageButton(const std::shared_ptr<ITexture>& normalImage, const std::shared_ptr<ITexture>& hoverImage,
                         glm::vec2 position,
                         glm::vec2 dimensions)
    : normalImage_(normalImage), hoverImage_(hoverImage)
{
    position_ = position;
    dimensions_ = dimensions;
}

void ImageButton::Render()
{
    if (visible_)
    {
        if (const std::shared_ptr<ITexture> currentImage = state_ == State::Hovered || state_ == State::Pressed
                                                               ? hoverImage_
                                                               : normalImage_)
        {
            Transform transform;
            transform.position = position_;
            Renderer::EnqueueImage(currentImage, transform, 1.0f, static_cast<int>(RenderLayer::UI));
        }
    }
}

std::shared_ptr<ITexture> ImageButton::GetNormalImage() const
{
    return normalImage_;
}

void ImageButton::SetNormalImage(const std::shared_ptr<ITexture>& normal_image)
{
    normalImage_ = normal_image;
}

std::shared_ptr<ITexture> ImageButton::GetHoverImage() const
{
    return hoverImage_;
}

void ImageButton::SetHoverImage(const std::shared_ptr<ITexture>& hover_image)
{
    hoverImage_ = hover_image;
}
