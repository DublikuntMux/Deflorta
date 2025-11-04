#pragma once

#include <memory>

#include "Button.hpp"
#include "../Render/IRenderBackend.hpp"

class ImageButton : public Button
{
public:
    ImageButton(
        std::shared_ptr<ITexture> normalImage,
        std::shared_ptr<ITexture> hoverImage,
        float x = 0.0f,
        float y = 0.0f,
        float width = 0.0f,
        float height = 0.0f
    );

    void Render() override;

    [[nodiscard]] std::shared_ptr<ITexture> GetNormalImage() const;
    void SetNormalImage(const std::shared_ptr<ITexture>& normal_image);

    [[nodiscard]] std::shared_ptr<ITexture> GetHoverImage() const;
    void SetHoverImage(const std::shared_ptr<ITexture>& hover_image);

private:
    std::shared_ptr<ITexture> normalImage_;
    std::shared_ptr<ITexture> hoverImage_;
};
