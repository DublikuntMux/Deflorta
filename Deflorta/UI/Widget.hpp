#pragma once

#include <glm/vec2.hpp>

class Widget
{
public:
    Widget() = default;
    virtual ~Widget() = default;

    virtual void Update()
    {
    }

    virtual void Render() = 0;

    void SetPosition(glm::vec2 position);
    [[nodiscard]] glm::vec2 GetPosition() const;

    [[nodiscard]] bool IsVisible() const;
    void SetVisible(bool visible);

protected:
    glm::vec2 position_ = {0.0f, 0.0f};
    bool visible_ = true;
};
