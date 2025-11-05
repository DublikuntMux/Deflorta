#include "Widget.hpp"

void Widget::SetPosition(glm::vec2 position)
{
    position_ = position;
}

glm::vec2 Widget::GetPosition() const
{
    return position_;
}

bool Widget::IsVisible() const
{
    return visible_;
}

void Widget::SetVisible(bool visible)
{
    visible_ = visible;
}
