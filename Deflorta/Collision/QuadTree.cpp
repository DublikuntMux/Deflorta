#include "QuadTree.hpp"

#include "../Object/GameObject.hpp"
#include "Collider.hpp"

QuadTree::QuadTree(int level, const Rect& bounds, int maxObjects, int maxLevels)
    : level_(level), bounds_(bounds), maxObjects_(maxObjects), maxLevels_(maxLevels)
{
    objects_.reserve(maxObjects);
}

void QuadTree::Clear()
{
    objects_.clear();
    
    for (auto& node : nodes_)
    {
        if (node)
        {
            node->Clear();
        }
    }
    
    nodes_.clear();
}

void QuadTree::Split()
{
    const float subWidth = bounds_.Width() / 2.0f;
    const float subHeight = bounds_.Height() / 2.0f;
    const float x = bounds_.X();
    const float y = bounds_.Y();

    nodes_.reserve(4);
    nodes_.push_back(std::make_unique<QuadTree>(level_ + 1, Rect::FromXYWH(x + subWidth, y, subWidth, subHeight), maxObjects_, maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(level_ + 1, Rect::FromXYWH(x, y, subWidth, subHeight), maxObjects_, maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(level_ + 1, Rect::FromXYWH(x, y + subHeight, subWidth, subHeight), maxObjects_, maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(level_ + 1, Rect::FromXYWH(x + subWidth, y + subHeight, subWidth, subHeight), maxObjects_, maxLevels_));
}

int QuadTree::GetIndex(GameObject* object) const
{
    if (!object) return -1;

    const Rect objectBounds = GetObjectBounds(object);
    
    const float verticalMidpoint = bounds_.X() + bounds_.Width() / 2.0f;
    const float horizontalMidpoint = bounds_.Y() + bounds_.Height() / 2.0f;

    const bool topQuadrant = objectBounds.Y() < horizontalMidpoint && 
        objectBounds.bottom < horizontalMidpoint;
    const bool bottomQuadrant = objectBounds.Y() > horizontalMidpoint;

    if (objectBounds.X() < verticalMidpoint && 
        objectBounds.right < verticalMidpoint)
    {
        if (topQuadrant)
        {
            return 1;
        }
        else if (bottomQuadrant)
        {
            return 2;
        }
    }
    else if (objectBounds.X() > verticalMidpoint)
    {
        if (topQuadrant)
        {
            return 0;
        }
        else if (bottomQuadrant)
        {
            return 3;
        }
    }

    return -1;
}

Rect QuadTree::GetObjectBounds(GameObject* object)
{
    if (!object) return Rect::FromXYWH(0, 0, 0, 0);

    const auto& transform = object->GetTransform();
    const glm::vec2 pos = transform.GetPosition();
    
    if (const auto* collider = object->GetCollider())
    {
        if (collider->GetType() == ColliderType::Box)
        {
            const auto* boxCollider = static_cast<const BoxCollider*>(collider);
            const glm::vec2 min = boxCollider->GetMin();
            const glm::vec2 max = boxCollider->GetMax();
            return Rect::FromXYWH(min.x, min.y, max.x - min.x, max.y - min.y);
        }
        else if (collider->GetType() == ColliderType::Circle)
        {
            const auto* circleCollider = static_cast<const CircleCollider*>(collider);
            const float radius = circleCollider->GetRadius();
            const glm::vec2 worldPos = collider->GetWorldPosition();
            return Rect::FromXYWH(worldPos.x - radius, worldPos.y - radius, radius * 2.0f, radius * 2.0f);
        }
    }

    return Rect::FromXYWH(pos.x - 0.5f, pos.y - 0.5f, 1.0f, 1.0f);
}

void QuadTree::Insert(GameObject* object)
{
    if (!object) return;

    if (!nodes_.empty())
    {
        const int index = GetIndex(object);
        if (index != -1)
        {
            nodes_[index]->Insert(object);
            return;
        }
    }

    objects_.push_back(object);

    if (objects_.size() > static_cast<size_t>(maxObjects_) && level_ < maxLevels_ && nodes_.empty())
    {
        Split();

        auto it = objects_.begin();
        while (it != objects_.end())
        {
            const int index = GetIndex(*it);
            if (index != -1)
            {
                nodes_[index]->Insert(*it);
                it = objects_.erase(it);
            }
            else
            {
                ++it;
            }
        }
    }
}

void QuadTree::Retrieve(std::vector<GameObject*>& returnObjects, GameObject* object) const
{
    if (!object) return;

    const int index = GetIndex(object);
    if (index != -1 && !nodes_.empty())
    {
        nodes_[index]->Retrieve(returnObjects, object);
    }
    else if (!nodes_.empty())
    {
        for (const auto& node : nodes_)
        {
            node->Retrieve(returnObjects, object);
        }
    }

    returnObjects.insert(returnObjects.end(), objects_.begin(), objects_.end());
}

void QuadTree::GetAllObjects(std::vector<GameObject*>& returnObjects) const
{
    returnObjects.insert(returnObjects.end(), objects_.begin(), objects_.end());
    
    for (const auto& node : nodes_)
    {
        if (node)
        {
            node->GetAllObjects(returnObjects);
        }
    }
}
