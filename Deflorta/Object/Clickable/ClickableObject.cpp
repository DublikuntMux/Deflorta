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
            wasPressed_ = false;
            OnMouseExit();
        }
        return;
    }

    HandleMouseInput();
}

void ClickableObject::HandleMouseInput()
{
    const auto mousePos = Input::GetMousePosition();
    const bool mouseOverNow = ContainsPoint(mousePos);

    if (mouseOverNow && !wasMouseOver_)
    {
        OnMouseEnter();
    }
    else if (!mouseOverNow && wasMouseOver_)
    {
        OnMouseExit();
    }

    if (mouseOverNow)
    {
        const bool leftMouseDown = Input::IsMouseDown(VK_LBUTTON);

        if (leftMouseDown)
        {
            if (!wasPressed_)
            {
                OnMouseDown();
            }
        }
        else
        {
            if (wasPressed_)
            {
                OnMouseUp();
                wasClicked_ = true;
                OnMouseClick();
            }
        }

        wasPressed_ = leftMouseDown;
    }
    else
    {
        wasPressed_ = false;
    }

    isMouseOver_ = mouseOverNow;
    wasMouseOver_ = mouseOverNow;

    if (isMouseOver_)
    {
        Input::SetCursorType(IDC_HAND);
    }
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

        return glm::all(glm::greaterThanEqual(point, min)) &&
            glm::all(glm::lessThanEqual(point, max));
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
