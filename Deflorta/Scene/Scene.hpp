#pragma once

#include <vector>

class Widget;

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

    virtual void Update() = 0;
    virtual void Render() = 0;

protected:
    void AddWidget(Widget* widget);
    void RemoveWidget(Widget* widget);
    void ClearWidgets();

    void UpdateWidgets() const;
    void RenderWidgets() const;

private:
    std::vector<Widget*> widgets_;
};
