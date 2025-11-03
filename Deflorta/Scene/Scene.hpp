#pragma once

#include <memory>
#include <vector>

class Widget;
class GameObject;

class Scene
{
public:
    virtual ~Scene() = default;

    virtual void OnEnter()
    {
    }

    virtual void OnExit()
    {
    }

    virtual void Update();
    virtual void Render();

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
};
