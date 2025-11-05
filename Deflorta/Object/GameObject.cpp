#include "GameObject.hpp"

#include "../Render/Renderer.hpp"

GameObject::GameObject(GameObjectTag tag) : tag_(tag)
{
    transform_ = Transform{
        .x = 0.0f,
        .y = 0.0f,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
    };
}

void GameObject::Render()
{
}

GameObjectTag GameObject::GetTag() const
{
    return tag_;
}

void GameObject::SetTag(GameObjectTag tag)
{
    tag_ = tag;
}

Transform& GameObject::GetTransform()
{
    return transform_;
}

const Transform& GameObject::GetTransform() const
{
    return transform_;
}

void GameObject::SetTransform(const Transform& transform)
{
    transform_ = transform;
}

bool GameObject::IsActive() const
{
    return isActive_;
}

void GameObject::SetActive(bool active)
{
    isActive_ = active;
}

void GameObject::SetCollider(std::unique_ptr<Collider> collider)
{
    collider_ = std::move(collider);
}
