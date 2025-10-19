#include "Widget.hpp"

D2D1_RECT_F Widget::GetReact() const
{
    return rect_;
}

void Widget::SetRect(const D2D1_RECT_F& rect)
{
    rect_ = rect;
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
