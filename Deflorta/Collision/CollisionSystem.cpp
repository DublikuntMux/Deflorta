#include "CollisionSystem.hpp"

#include "../Object/GameObject.hpp"
#include "Collider.hpp"

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
