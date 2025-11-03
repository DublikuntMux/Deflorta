#include "Scene.hpp"

#include <algorithm>

#include "../Object/GameObject.hpp"
#include "../UI/Widget.hpp"

void Scene::Update()
{
    for (const auto& gameObject : gameObjects_)
    {
        if (gameObject && gameObject->IsActive())
        {
            gameObject->Update();
        }
    }

    for (auto* widget : widgets_)
    {
        if (widget && widget->IsVisible())
        {
            widget->Update();
        }
    }
}

void Scene::Render()
{
    for (const auto& gameObject : gameObjects_)
    {
        if (gameObject && gameObject->IsActive())
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
