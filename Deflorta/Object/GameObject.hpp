#pragma once

#include <memory>

#include "../Base/Transform.hpp"
#include "../Collision/Collider.hpp"

enum class GameObjectTag : std::uint8_t
{
    None,
    Plant,
    Bush,
    Fog,
    Zombie,
    Projectile,
    Sun,
    Coin
};

class Collider;

class GameObject
{
public:
    explicit GameObject(GameObjectTag tag = GameObjectTag::None);
    virtual ~GameObject() = default;

    virtual void Update() = 0;
    virtual void Render();

    virtual void OnCollisionEnter(GameObject* other) {}
    virtual void OnCollisionStay(GameObject* other) {}
    virtual void OnCollisionExit(GameObject* other) {}

    [[nodiscard]] GameObjectTag GetTag() const;
    void SetTag(GameObjectTag tag);

    [[nodiscard]] Transform& GetTransform();
    [[nodiscard]] const Transform& GetTransform() const;
    void SetTransform(const Transform& transform);

    [[nodiscard]] bool IsActive() const;
    void SetActive(bool active);

    [[nodiscard]] Collider* GetCollider() const { return collider_.get(); }
    void SetCollider(std::unique_ptr<Collider> collider);
    template<typename T, typename... Args>
    T* AddCollider(Args&&... args);

protected:
    Transform transform_;
    GameObjectTag tag_;
    bool isActive_ = true;
    std::unique_ptr<Collider> collider_;
};

template<typename T, typename... Args>
T* GameObject::AddCollider(Args&&... args)
{
    static_assert(std::is_base_of_v<Collider, T>, "T must derive from Collider");
    auto collider = std::make_unique<T>(this, std::forward<Args>(args)...);
    T* ptr = collider.get();
    collider_ = std::move(collider);
    return ptr;
}
