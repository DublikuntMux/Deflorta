#pragma once

#include "Button.hpp"
#include "../Render/IRenderBackend.hpp"

#include <memory>

class ImageButton : public Button
{
public:
    ImageButton(
        const std::shared_ptr<ITexture>& normalImage,
        const std::shared_ptr<ITexture>& hoverImage,
        glm::vec2 position,
        glm::vec2 dimensions = {0.0f, 0.0f}
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
