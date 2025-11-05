#include "QuadTree.hpp"

#include "../Object/GameObject.hpp"
#include "../Render/Renderer.hpp"
#include "Collider.hpp"
#include "../Render/Layer.hpp"

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
    const glm::vec2 size = bounds_.Size();
    const glm::vec2 halfSize = size * 0.5f;
    const glm::vec2 min = bounds_.min;

    nodes_.reserve(4);
    nodes_.push_back(std::make_unique<QuadTree>(
        level_ + 1, Rect::FromXYWH(min.x + halfSize.x, min.y, halfSize.x, halfSize.y),
        maxObjects_, maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(level_ + 1, Rect::FromXYWH(min.x, min.y, halfSize.x, halfSize.y),
                                                maxObjects_,
                                                maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(
        level_ + 1, Rect::FromXYWH(min.x, min.y + halfSize.y, halfSize.x, halfSize.y),
        maxObjects_, maxLevels_));
    nodes_.push_back(std::make_unique<QuadTree>(
        level_ + 1, Rect::FromXYWH(min.x + halfSize.x, min.y + halfSize.y, halfSize.x, halfSize.y), maxObjects_,
        maxLevels_));
}

int QuadTree::GetIndex(GameObject* object) const
{
    if (!object) return -1;

    const Rect objectBounds = GetObjectBounds(object);

    const glm::vec2 midpoint = bounds_.Center();

    const bool inLeftQuadrant = objectBounds.max.x <= midpoint.x;
    const bool inRightQuadrant = objectBounds.min.x >= midpoint.x;
    const bool inTopQuadrant = objectBounds.max.y <= midpoint.y;
    const bool inBottomQuadrant = objectBounds.min.y >= midpoint.y;

    if (inRightQuadrant)
    {
        if (inTopQuadrant)
        {
            return 0;
        }
        if (inBottomQuadrant)
        {
            return 3;
        }
    }
    else if (inLeftQuadrant)
    {
        if (inTopQuadrant)
        {
            return 1;
        }
        if (inBottomQuadrant)
        {
            return 2;
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
            const auto* boxCollider = dynamic_cast<const BoxCollider*>(collider);
            const glm::vec2 min = boxCollider->GetMin();
            const glm::vec2 max = boxCollider->GetMax();
            return Rect::FromXYWH(min.x, min.y, max.x - min.x, max.y - min.y);
        }
        if (collider->GetType() == ColliderType::Circle)
        {
            const auto* circleCollider = dynamic_cast<const CircleCollider*>(collider);
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

void QuadTree::DebugRender(int depth) const
{
    const float alpha = 0.3f - static_cast<float>(depth) * 0.1f;
    const Color color(0.0f, 1.0f, 1.0f, std::max(0.1f, alpha));

    Renderer::EnqueueRectangle(bounds_, color, 1.0f, false, static_cast<int>(RenderLayer::Debug));

    for (const auto& node : nodes_)
    {
        if (node)
        {
            node->DebugRender(depth + 1);
        }
    }
}
