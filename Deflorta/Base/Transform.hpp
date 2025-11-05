#pragma once

#include <glm/glm.hpp>

struct Transform
{
    glm::vec2 position = {0.0f, 0.0f};
    glm::vec2 scale = {1.0f, 1.0f};
    float rotation = 0.0f;

    [[nodiscard]] glm::vec2 GetPosition() const { return position; }

    void SetPosition(const glm::vec2& pos)
    {
        position = pos;
    }

    [[nodiscard]] glm::vec2 GetScale() const { return scale; }

    void SetScale(const glm::vec2& newScale)
    {
        scale = newScale;
    }
};
