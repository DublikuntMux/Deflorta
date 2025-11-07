#include "ImageButton.hpp"

#include "../Base/Transform.hpp"
#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"

ImageButton::ImageButton(std::shared_ptr<ITexture> normalImage, std::shared_ptr<ITexture> hoverImage,
                         glm::vec2 position,
                         glm::vec2 dimensions)
    : normalImage_(std::move(normalImage)), hoverImage_(std::move(hoverImage))
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

void ImageButton::SetNormalImage(std::shared_ptr<ITexture> normal_image)
{
    normalImage_ = std::move(normal_image);
}

std::shared_ptr<ITexture> ImageButton::GetHoverImage() const
{
    return hoverImage_;
}

void ImageButton::SetHoverImage(std::shared_ptr<ITexture> hover_image)
{
    hoverImage_ = std::move(hover_image);
}
