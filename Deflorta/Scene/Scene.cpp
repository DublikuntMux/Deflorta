#include "Scene.hpp"

#include <algorithm>

#include "../UI/Widget.hpp"

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

void Scene::UpdateWidgets() const
{
    for (auto* widget : widgets_)
    {
        if (widget && widget->IsVisible())
        {
            widget->Update();
        }
    }
}

void Scene::RenderWidgets() const
{
    for (auto* widget : widgets_)
    {
        if (widget && widget->IsVisible())
        {
            widget->Render();
        }
    }
}
