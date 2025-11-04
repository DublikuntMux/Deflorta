#pragma once

#include <glm/glm.hpp>

struct Transform
{
    float x = 0.0f;
    float y = 0.0f;
    float rotation = 0.0f;
    float scaleX = 1.0f;
    float scaleY = 1.0f;

    glm::vec2 GetPosition() const { return glm::vec2(x, y); }

    void SetPosition(const glm::vec2& pos)
    {
        x = pos.x;
        y = pos.y;
    }

    glm::vec2 GetScale() const { return glm::vec2(scaleX, scaleY); }

    void SetScale(const glm::vec2& scale)
    {
        scaleX = scale.x;
        scaleY = scale.y;
    }
};
