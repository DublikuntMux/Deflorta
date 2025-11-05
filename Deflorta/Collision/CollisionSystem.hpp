#pragma once

#include "QuadTree.hpp"
#include "CollisionMatrix.hpp"
#include "../Base/Color.hpp"

#include <vector>
#include <memory>
#include <unordered_set>

class GameObject;

struct CollisionPair
{
    GameObject* obj1;
    GameObject* obj2;

    bool operator==(const CollisionPair& other) const
    {
        return (obj1 == other.obj1 && obj2 == other.obj2) ||
            (obj1 == other.obj2 && obj2 == other.obj1);
    }

    struct Hash
    {
        size_t operator()(const CollisionPair& pair) const
        {
            const size_t h1 = std::hash<GameObject*>{}(pair.obj1);
            const size_t h2 = std::hash<GameObject*>{}(pair.obj2);
            return h1 < h2 ? h1 ^ h2 << 1 : h2 ^ h1 << 1;
        }
    };
};

class CollisionSystem
{
public:
    explicit CollisionSystem(const Rect& worldBounds);

    void Update(const std::vector<std::shared_ptr<GameObject>>& objects);
    void DebugRender() const;

    [[nodiscard]] CollisionMatrix& GetCollisionMatrix() { return collisionMatrix_; }
    [[nodiscard]] const CollisionMatrix& GetCollisionMatrix() const { return collisionMatrix_; }

    void SetDebugRenderEnabled(bool enabled) { debugRenderEnabled_ = enabled; }
    [[nodiscard]] bool IsDebugRenderEnabled() const { return debugRenderEnabled_; }

private:
    std::unique_ptr<QuadTree> quadTree_;
    CollisionMatrix collisionMatrix_;

    std::unordered_set<CollisionPair, CollisionPair::Hash> previousCollisions_;
    std::unordered_set<CollisionPair, CollisionPair::Hash> currentCollisions_;

    bool debugRenderEnabled_ = false;

    [[nodiscard]] bool ShouldCollide(GameObject* obj1, GameObject* obj2) const;
    void ProcessCollision(GameObject* obj1, GameObject* obj2);
    static Color GetColorForTag(GameObjectTag tag);
};
