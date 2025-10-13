#include "GameObject.hpp"

Transform& GameObject::transform() noexcept
{
    return transform_;
}

const Transform& GameObject::transform() const noexcept
{
    return transform_;
}
