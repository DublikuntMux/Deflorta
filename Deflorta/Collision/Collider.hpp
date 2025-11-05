#pragma once

#include <glm/glm.hpp>

enum class ColliderType : std::uint8_t
{
    Box,
    Circle
};

class GameObject;

class Collider
{
public:
    explicit Collider(GameObject* owner, ColliderType type)
        : owner_(owner), type_(type) {}
    virtual ~Collider() = default;

    [[nodiscard]] ColliderType GetType() const { return type_; }
    [[nodiscard]] GameObject* GetOwner() const { return owner_; }
    
    [[nodiscard]] bool IsEnabled() const { return enabled_; }
    void SetEnabled(bool enabled) { enabled_ = enabled; }

    [[nodiscard]] bool IsTrigger() const { return isTrigger_; }
    void SetTrigger(bool trigger) { isTrigger_ = trigger; }

    [[nodiscard]] const glm::vec2& GetOffset() const { return offset_; }
    void SetOffset(const glm::vec2& offset) { offset_ = offset; }

    [[nodiscard]] virtual glm::vec2 GetWorldPosition() const;

    virtual bool Intersects(const Collider* other) const = 0;
    virtual bool IntersectsBox(const class BoxCollider* box) const = 0;
    virtual bool IntersectsCircle(const class CircleCollider* circle) const = 0;

protected:
    GameObject* owner_;
    ColliderType type_;
    glm::vec2 offset_ = glm::vec2(0.0f, 0.0f);
    bool enabled_ = true;
    bool isTrigger_ = false;
};

class BoxCollider : public Collider
{
public:
    explicit BoxCollider(GameObject* owner, float width = 1.0f, float height = 1.0f)
        : Collider(owner, ColliderType::Box), width_(width), height_(height) {}

    [[nodiscard]] float GetWidth() const { return width_; }
    void SetWidth(float width) { width_ = width; }

    [[nodiscard]] float GetHeight() const { return height_; }
    void SetHeight(float height) { height_ = height; }

    void SetSize(float width, float height)
    {
        width_ = width;
        height_ = height;
    }

    [[nodiscard]] glm::vec2 GetMin() const;
    [[nodiscard]] glm::vec2 GetMax() const;

    bool Intersects(const Collider* other) const override;
    bool IntersectsBox(const BoxCollider* box) const override;
    bool IntersectsCircle(const CircleCollider* circle) const override;

private:
    float width_;
    float height_;
};

class CircleCollider : public Collider
{
public:
    explicit CircleCollider(GameObject* owner, float radius = 1.0f)
        : Collider(owner, ColliderType::Circle), radius_(radius) {}

    [[nodiscard]] float GetRadius() const { return radius_; }
    void SetRadius(float radius) { radius_ = radius; }

    bool Intersects(const Collider* other) const override;
    bool IntersectsBox(const BoxCollider* box) const override;
    bool IntersectsCircle(const CircleCollider* circle) const override;

private:
    float radius_;
};
