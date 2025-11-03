#pragma once

class Widget
{
public:
    Widget() = default;
    virtual ~Widget() = default;

    virtual void Update()
    {
    }

    virtual void Render() = 0;

    [[nodiscard]] float GetX() const;
    void SetX(float x);

    [[nodiscard]] float GetY() const;
    void SetY(float y);

    void SetPosition(float x, float y);

    [[nodiscard]] bool IsVisible() const;
    void SetVisible(bool visible);

protected:
    float x_ = 0.0f;
    float y_ = 0.0f;
    bool visible_ = true;
};
