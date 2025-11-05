#pragma once

#include "../GameObject.hpp"

class ClickableObject : public GameObject
{
public:
    explicit ClickableObject(GameObjectTag tag = GameObjectTag::None);

    void Update() override;

    virtual void OnMouseEnter()
    {
    }

    virtual void OnMouseExit()
    {
    }

    virtual void OnMouseDown()
    {
    }

    virtual void OnMouseUp()
    {
    }

    virtual void OnMouseClick()
    {
    }

    [[nodiscard]] bool IsMouseOver() const { return isMouseOver_; }
    [[nodiscard]] bool WasClicked() const { return wasClicked_; }

    void SetClickable(bool clickable) { isClickable_ = clickable; }
    [[nodiscard]] bool IsClickable() const { return isClickable_; }

protected:
    [[nodiscard]] bool ContainsPoint(const glm::vec2& point) const;

private:
    bool isMouseOver_ = false;
    bool wasMouseOver_ = false;
    bool isMouseDown_ = false;
    bool wasClicked_ = false;
    bool isClickable_ = true;

    void HandleMouseInput();
};
