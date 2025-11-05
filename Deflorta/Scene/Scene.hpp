#pragma once

#include "../Collision/CollisionSystem.hpp"

#include <memory>
#include <vector>

class Widget;
class GameObject;

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

protected:
    void AddWidget(Widget* widget);
    void RemoveWidget(Widget* widget);
    void ClearWidgets();

    void AddGameObject(const std::shared_ptr<GameObject>& gameObject);
    void RemoveGameObject(const std::shared_ptr<GameObject>& gameObject);
    void ClearGameObjects();

private:
    std::vector<Widget*> widgets_;
    std::vector<std::shared_ptr<GameObject>> gameObjects_;
    std::unique_ptr<CollisionSystem> collisionSystem_;
};
