#include "GameObject.hpp"

Transform& GameObject::GetTransform() noexcept
{
    return transform_;
}

const Transform& GameObject::GetTransform() const noexcept
{
    return transform_;
}
