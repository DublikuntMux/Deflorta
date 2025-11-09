#include "Scene.hpp"

#include "../Object/GameObject.hpp"
#include "../Collision/CollisionSystem.hpp"
#include "../Collision/QuadTree.hpp"
#include "../UI/Widget.hpp"

#include <algorithm>

Scene::Scene()
{
    collisionSystem_ = std::make_unique<CollisionSystem>(Rect::FromXYWH(0, 0, 1280, 720));
    gameObjects_.reserve(128);
}

void Scene::Update()
{
    for (auto& gameObject : addPool_)
    {
        gameObjects_.push_back(std::move(gameObject));
    }
    addPool_.clear();

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

    for (const auto& gameObject : deletePool_)
    {
        std::erase(gameObjects_, gameObject);
    }
    deletePool_.clear();
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

void Scene::QueueFree(std::shared_ptr<GameObject> gameObject)
{
    deletePool_.emplace_back(std::move(gameObject));
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

void Scene::AddGameObject(std::shared_ptr<GameObject> gameObject)
{
    if (gameObject)
    {
        gameObject->scene_ = this;
        addPool_.push_back(std::move(gameObject));
    }
}

void Scene::ClearGameObjects()
{
    gameObjects_.clear();
}
