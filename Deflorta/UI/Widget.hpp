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

    [[nodiscard]] int GetZ() const;
    void SetZ(int z);

    [[nodiscard]] bool IsVisible() const;
    void SetVisible(bool visible);

protected:
    float x_ = 0.0f;
    float y_ = 0.0f;
    int zOrder_ = 0;
    bool visible_ = true;
};
