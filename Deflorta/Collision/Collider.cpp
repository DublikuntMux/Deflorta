#include "Collider.hpp"

#include "../Object/GameObject.hpp"

#include <algorithm>

glm::vec2 Collider::GetWorldPosition() const
{
    if (owner_)
    {
        return owner_->GetTransform().GetPosition() + offset_;
    }
    return offset_;
}

glm::vec2 BoxCollider::GetMin() const
{
    const glm::vec2 pos = GetWorldPosition();
    return pos - glm::vec2(width_ * 0.5f, height_ * 0.5f);
}

glm::vec2 BoxCollider::GetMax() const
{
    const glm::vec2 pos = GetWorldPosition();
    return pos + glm::vec2(width_ * 0.5f, height_ * 0.5f);
}

bool BoxCollider::Intersects(const Collider* other) const
{
    if (!other || !enabled_ || !other->IsEnabled())
        return false;

    if (other->GetType() == ColliderType::Box)
    {
        return IntersectsBox(dynamic_cast<const BoxCollider*>(other));
    }
    if (other->GetType() == ColliderType::Circle)
    {
        return IntersectsCircle(dynamic_cast<const CircleCollider*>(other));
    }
    return false;
}

bool BoxCollider::IntersectsBox(const BoxCollider* box) const
{
    if (!box) return false;

    const glm::vec2 min1 = GetMin();
    const glm::vec2 max1 = GetMax();
    const glm::vec2 min2 = box->GetMin();
    const glm::vec2 max2 = box->GetMax();

    return min1.x <= max2.x && max1.x >= min2.x &&
        min1.y <= max2.y && max1.y >= min2.y;
}

bool BoxCollider::IntersectsCircle(const CircleCollider* circle) const
{
    if (!circle) return false;

    const glm::vec2 boxCenter = GetWorldPosition();
    const glm::vec2 circleCenter = circle->GetWorldPosition();
    const float radius = circle->GetRadius();

    const float halfWidth = width_ * 0.5f;
    const float halfHeight = height_ * 0.5f;

    const glm::vec2 delta = circleCenter - boxCenter;
    const auto closest = glm::vec2(
        std::clamp(delta.x, -halfWidth, halfWidth),
        std::clamp(delta.y, -halfHeight, halfHeight)
    );

    const glm::vec2 closestPoint = boxCenter + closest;
    const glm::vec2 diff = circleCenter - closestPoint;
    const float distSq = glm::dot(diff, diff);

    return distSq <= radius * radius;
}

bool CircleCollider::Intersects(const Collider* other) const
{
    if (!other || !enabled_ || !other->IsEnabled())
        return false;

    if (other->GetType() == ColliderType::Box)
    {
        return IntersectsBox(dynamic_cast<const BoxCollider*>(other));
    }
    if (other->GetType() == ColliderType::Circle)
    {
        return IntersectsCircle(dynamic_cast<const CircleCollider*>(other));
    }
    return false;
}

bool CircleCollider::IntersectsBox(const BoxCollider* box) const
{
    if (!box) return false;
    return box->IntersectsCircle(this);
}

bool CircleCollider::IntersectsCircle(const CircleCollider* circle) const
{
    if (!circle) return false;

    const glm::vec2 pos1 = GetWorldPosition();
    const glm::vec2 pos2 = circle->GetWorldPosition();
    const float radiusSum = radius_ + circle->GetRadius();

    const glm::vec2 delta = pos2 - pos1;
    const float distSq = glm::dot(delta, delta);

    return distSq <= radiusSum * radiusSum;
}
