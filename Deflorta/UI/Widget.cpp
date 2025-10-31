#include "Widget.hpp"

float Widget::GetX() const
{
    return x_;
}

void Widget::SetX(float x)
{
    x_ = x;
}

float Widget::GetY() const
{
    return y_;
}

void Widget::SetY(float y)
{
    y_ = y;
}

void Widget::SetPosition(float x, float y)
{
    x_ = x;
    y_ = y;
}

int Widget::GetZ() const
{
    return zOrder_;
}

void Widget::SetZ(int z)
{
    zOrder_ = z;
}

bool Widget::IsVisible() const
{
    return visible_;
}

void Widget::SetVisible(bool visible)
{
    visible_ = visible;
}
