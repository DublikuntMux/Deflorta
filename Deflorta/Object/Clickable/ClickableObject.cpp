#include "ClickableObject.hpp"

#include "../../Collision/Collider.hpp"
#include "../../Base/Input.hpp"

#include <glm/glm.hpp>

ClickableObject::ClickableObject(GameObjectTag tag)
    : GameObject(tag)
{
}

void ClickableObject::Update()
{
    wasClicked_ = false;

    if (!isClickable_ || !isActive_)
    {
        if (isMouseOver_)
        {
            isMouseOver_ = false;
            wasMouseOver_ = false;
            isMouseDown_ = false;
            OnMouseExit();
        }
        return;
    }

    HandleMouseInput();
}

void ClickableObject::HandleMouseInput()
{
    const auto mousePos = Input::GetMousePosition();
    const bool mouseOverNow = ContainsPoint(mousePos.x, mousePos.y);

    if (mouseOverNow && !wasMouseOver_)
    {
        isMouseOver_ = true;
        OnMouseEnter();
    }
    else if (!mouseOverNow && wasMouseOver_)
    {
        isMouseOver_ = false;
        OnMouseExit();

        if (isMouseDown_)
        {
            isMouseDown_ = false;
        }
    }
    else
    {
        isMouseOver_ = mouseOverNow;
    }

    if (isMouseOver_)
    {
        const bool leftMouseDown = Input::IsMouseDown(VK_LBUTTON);
        const bool leftMousePressed = Input::IsMousePressed(VK_LBUTTON);

        if (leftMousePressed && !isMouseDown_)
        {
            isMouseDown_ = true;
            OnMouseDown();
        }

        if (!leftMouseDown && isMouseDown_)
        {
            isMouseDown_ = false;
            OnMouseUp();

            wasClicked_ = true;
            OnMouseClick();
        }
    }
    else
    {
        if (isMouseDown_)
        {
            isMouseDown_ = false;
        }
    }

    wasMouseOver_ = isMouseOver_;
}

bool ClickableObject::ContainsPoint(float x, float y) const
{
    return ContainsPoint(glm::vec2(x, y));
}

bool ClickableObject::ContainsPoint(const glm::vec2& point) const
{
    if (!collider_ || !collider_->IsEnabled())
        return false;

    if (collider_->GetType() == ColliderType::Box)
    {
        const auto* boxCollider = dynamic_cast<const BoxCollider*>(collider_.get());
        if (!boxCollider) return false;

        const glm::vec2 min = boxCollider->GetMin();
        const glm::vec2 max = boxCollider->GetMax();

        return point.x >= min.x && point.x <= max.x &&
            point.y >= min.y && point.y <= max.y;
    }

    if (collider_->GetType() == ColliderType::Circle)
    {
        const auto* circleCollider = dynamic_cast<const CircleCollider*>(collider_.get());
        if (!circleCollider) return false;

        const glm::vec2 center = circleCollider->GetWorldPosition();
        const float radius = circleCollider->GetRadius();

        const glm::vec2 delta = point - center;
        const float distSq = glm::dot(delta, delta);

        return distSq <= radius * radius;
    }

    return false;
}
