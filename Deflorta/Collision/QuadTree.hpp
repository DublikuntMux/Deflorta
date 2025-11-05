#pragma once

#include <vector>
#include <memory>

#include "../Render/RenderTypes.hpp"

class GameObject;
class Collider;

class QuadTree
{
public:
    explicit QuadTree(int level, const Rect& bounds, int maxObjects = 10, int maxLevels = 5);
    ~QuadTree() = default;

    void Clear();
    void Insert(GameObject* object);
    void Retrieve(std::vector<GameObject*>& returnObjects, GameObject* object) const;
    void GetAllObjects(std::vector<GameObject*>& returnObjects) const;
    void DebugRender(int depth = 0) const;

private:
    int level_;
    Rect bounds_;
    std::vector<GameObject*> objects_;
    std::vector<std::unique_ptr<QuadTree>> nodes_;
    int maxObjects_;
    int maxLevels_;

    void Split();
    int GetIndex(GameObject* object) const;
    static Rect GetObjectBounds(GameObject* object);
};
