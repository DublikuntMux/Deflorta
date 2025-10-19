#pragma once

#include <d2d1.h>

class Widget
{
public:
    Widget() = default;
    virtual ~Widget() = default;

    virtual void Update()
    {
    }

    virtual void Render() = 0;

    [[nodiscard]] D2D1_RECT_F GetReact() const;
    void SetRect(const D2D1_RECT_F& rect);

    [[nodiscard]] int GetZ() const;
    void SetZ(int z);

    [[nodiscard]] bool IsVisible() const;
    void SetVisible(bool visible);

protected:
    D2D1_RECT_F rect_{};
    int zOrder_ = 0;
    bool visible_ = true;
};
