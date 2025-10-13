#pragma once

#include "Transform.hpp"

class GameObject
{
public:
    virtual ~GameObject() = default;

    virtual void Update(float dt)
    {
    }

    virtual void Render()
    {
    }

    [[nodiscard]] Transform& GetTransform() noexcept;
    [[nodiscard]] const Transform& GetTransform() const noexcept;

protected:
    Transform transform_;
};
