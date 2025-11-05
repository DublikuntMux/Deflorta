#include "GameObject.hpp"

#include "../Render/Renderer.hpp"

GameObject::GameObject(GameObjectTag tag) : tag_(tag)
{
    transform_ = Transform{
        .position = {0.0f, 0.0f},
        .scale = {1.0f, 1.0f},
        .rotation = 0.0f,
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
