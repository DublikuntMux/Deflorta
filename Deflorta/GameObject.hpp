#pragma once

#include "Renderer.hpp"
#include "Transform.hpp"

class GameObject
{
public:
    virtual ~GameObject() = default;

    virtual void update(float dt)
    {
    }

    virtual void render(Renderer& renderer)
    {
    }

    Transform& transform() noexcept;
    [[nodiscard]] const Transform& transform() const noexcept;

protected:
    Transform transform_;
};
