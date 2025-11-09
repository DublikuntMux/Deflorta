#pragma once

#include "../Collision/CollisionSystem.hpp"

#include <memory>
#include <vector>

class Widget;
class GameObject;
class Timer;

class Scene
{
public:
    Scene();
    virtual ~Scene() = default;

    virtual void OnEnter()
    {
    }

    virtual void OnExit()
    {
    }

    virtual void Update();
    virtual void Render();

    [[nodiscard]] CollisionSystem* GetCollisionSystem() const { return collisionSystem_.get(); }

    void QueueFree(std::shared_ptr<GameObject> gameObject);
    void AddGameObject(std::shared_ptr<GameObject> gameObject);

    void AddWidget(Widget* widget);
    void RemoveWidget(Widget* widget);

protected:
    void ClearWidgets();
    void ClearGameObjects();

private:
    std::vector<Widget*> widgets_;
    std::vector<std::shared_ptr<GameObject>> gameObjects_;
    std::vector<std::shared_ptr<GameObject>> deletePool_;
    std::vector<std::shared_ptr<GameObject>> addPool_;
    std::unique_ptr<CollisionSystem> collisionSystem_;
};
