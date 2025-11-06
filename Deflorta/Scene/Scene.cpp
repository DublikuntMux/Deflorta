#include "Scene.hpp"

#include "../Object/GameObject.hpp"
#include "../Collision/CollisionSystem.hpp"
#include "../Collision/QuadTree.hpp"
#include "../UI/Widget.hpp"

#include <algorithm>

Scene::Scene()
{
    collisionSystem_ = std::make_unique<CollisionSystem>(Rect::FromXYWH(0, 0, 1280, 720));
}

void Scene::Update()
{
    for (const auto& gameObject : gameObjects_)
    {
        if (gameObject && gameObject->IsActive())
        {
            gameObject->Update();
        }
    }

    if (collisionSystem_)
    {
        collisionSystem_->Update(gameObjects_);
    }

    for (auto* widget : widgets_)
    {
        if (widget && widget->IsVisible())
        {
            widget->Update();
        }
    }

    std::erase_if(gameObjects_,
                  [](const std::shared_ptr<GameObject>& obj) { return obj && obj->isQueuedForDeletion_; });
}

void Scene::Render()
{
    for (const auto& gameObject : gameObjects_)
    {
        if (gameObject && gameObject->IsVisible())
        {
            gameObject->Render();
        }
    }

    for (auto* widget : widgets_)
    {
        if (widget && widget->IsVisible())
        {
            widget->Render();
        }
    }

    if (collisionSystem_)
    {
        collisionSystem_->DebugRender();
    }
}

void Scene::AddWidget(Widget* widget)
{
    if (widget)
    {
        widgets_.push_back(widget);
    }
}

void Scene::RemoveWidget(Widget* widget)
{
    const auto it = std::ranges::find(widgets_, widget);
    if (it != widgets_.end())
    {
        widgets_.erase(it);
    }
}

void Scene::ClearWidgets()
{
    widgets_.clear();
}

void Scene::AddGameObject(const std::shared_ptr<GameObject>& gameObject)
{
    if (gameObject)
    {
        gameObject->scene_ = this;
        gameObjects_.push_back(gameObject);
    }
}

void Scene::RemoveGameObject(const std::shared_ptr<GameObject>& gameObject)
{
    const auto it = std::ranges::find(gameObjects_, gameObject);
    if (it != gameObjects_.end())
    {
        gameObjects_.erase(it);
    }
}

void Scene::ClearGameObjects()
{
    gameObjects_.clear();
}
