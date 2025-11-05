#include "CollisionSystem.hpp"

#include "../Object/GameObject.hpp"
#include "../Render/Renderer.hpp"
#include "Collider.hpp"
#include "../Render/Layer.hpp"

CollisionSystem::CollisionSystem(const Rect& worldBounds)
{
    quadTree_ = std::make_unique<QuadTree>(0, worldBounds);
}

bool CollisionSystem::ShouldCollide(GameObject* obj1, GameObject* obj2) const
{
    if (!obj1 || !obj2) return false;
    if (obj1 == obj2) return false;
    if (!obj1->IsActive() || !obj2->IsActive()) return false;

    const auto* collider1 = obj1->GetCollider();
    const auto* collider2 = obj2->GetCollider();
    if (!collider1 || !collider2) return false;
    if (!collider1->IsEnabled() || !collider2->IsEnabled()) return false;

    return collisionMatrix_.CanCollide(obj1->GetTag(), obj2->GetTag());
}

void CollisionSystem::ProcessCollision(GameObject* obj1, GameObject* obj2)
{
    if (!ShouldCollide(obj1, obj2)) return;

    const auto* collider1 = obj1->GetCollider();
    const auto* collider2 = obj2->GetCollider();

    if (collider1->Intersects(collider2))
    {
        const CollisionPair pair{.obj1 = obj1, .obj2 = obj2};
        currentCollisions_.insert(pair);

        if (previousCollisions_.contains(pair))
        {
            obj1->OnCollisionStay(obj2);
            obj2->OnCollisionStay(obj1);
        }
        else
        {
            obj1->OnCollisionEnter(obj2);
            obj2->OnCollisionEnter(obj1);
        }
    }
}

void CollisionSystem::Update(const std::vector<std::shared_ptr<GameObject>>& objects)
{
    quadTree_->Clear();

    for (const auto& obj : objects)
    {
        if (obj && obj->IsActive() && obj->GetCollider() && obj->GetCollider()->IsEnabled())
        {
            quadTree_->Insert(obj.get());
        }
    }

    currentCollisions_.clear();

    for (const auto& obj : objects)
    {
        if (!obj || !obj->IsActive()) continue;
        if (!obj->GetCollider() || !obj->GetCollider()->IsEnabled()) continue;

        std::vector<GameObject*> candidates;
        quadTree_->Retrieve(candidates, obj.get());

        for (auto* candidate : candidates)
        {
            if (candidate != obj.get())
            {
                ProcessCollision(obj.get(), candidate);
            }
        }
    }

    for (const auto& pair : previousCollisions_)
    {
        if (!currentCollisions_.contains(pair))
        {
            if (pair.obj1 && pair.obj2)
            {
                pair.obj1->OnCollisionExit(pair.obj2);
                pair.obj2->OnCollisionExit(pair.obj1);
            }
        }
    }

    previousCollisions_ = currentCollisions_;
}

void CollisionSystem::DebugRender() const
{
    if (!debugRenderEnabled_) return;

    if (quadTree_)
    {
        quadTree_->DebugRender();
    }

    std::vector<GameObject*> allObjects;
    if (quadTree_)
    {
        quadTree_->GetAllObjects(allObjects);
    }

    for (const auto* obj : allObjects)
    {
        if (!obj || !obj->IsActive()) continue;

        const auto* collider = obj->GetCollider();
        if (!collider || !collider->IsEnabled()) continue;

        const Color color = GetColorForTag(obj->GetTag());

        if (collider->GetType() == ColliderType::Box)
        {
            const auto* boxCollider = dynamic_cast<const BoxCollider*>(collider);
            const glm::vec2 min = boxCollider->GetMin();
            const glm::vec2 max = boxCollider->GetMax();
            const Rect bounds = Rect::FromXYWH(min.x, min.y, max.x - min.x, max.y - min.y);
            Renderer::EnqueueRectangle(bounds, color, 2.0f, false, static_cast<int>(RenderLayer::Debug));
        }
        else if (collider->GetType() == ColliderType::Circle)
        {
            const auto* circleCollider = dynamic_cast<const CircleCollider*>(collider);
            const glm::vec2 worldPos = circleCollider->GetWorldPosition();
            const float radius = circleCollider->GetRadius();

            const Rect bounds = Rect::FromXYWH(
                worldPos.x - radius,
                worldPos.y - radius,
                radius * 2.0f,
                radius * 2.0f);
            Renderer::EnqueueRectangle(bounds, color, 2.0f, false, static_cast<int>(RenderLayer::Debug));

            const Rect centerH = Rect::FromXYWH(worldPos.x - radius, worldPos.y - 1.0f, radius * 2.0f, 2.0f);
            const Rect centerV = Rect::FromXYWH(worldPos.x - 1.0f, worldPos.y - radius, 2.0f, radius * 2.0f);
            Renderer::EnqueueRectangle(centerH, color, 1.0f, true, static_cast<int>(RenderLayer::Debug));
            Renderer::EnqueueRectangle(centerV, color, 1.0f, true, static_cast<int>(RenderLayer::Debug));
        }
    }
}

Color CollisionSystem::GetColorForTag(GameObjectTag tag)
{
    switch (tag)
    {
    case GameObjectTag::None:
        return {1.0f, 1.0f, 1.0f, 0.7f};
    case GameObjectTag::Plant:
        return {0.0f, 1.0f, 0.0f, 0.7f};
    case GameObjectTag::Zombie:
        return {1.0f, 0.0f, 0.0f, 0.7f};
    case GameObjectTag::Projectile:
        return {1.0f, 1.0f, 0.0f, 0.7f};
    case GameObjectTag::Sun:
        return {1.0f, 0.8f, 0.0f, 0.7f};
    case GameObjectTag::Coin:
        return {1.0f, 0.843f, 0.0f, 0.7f};
    default:
        return {1.0f, 1.0f, 1.0f, 0.7f};
    }
}
