#pragma once

#include "../Base/Transform.hpp"
#include "../Collision/Collider.hpp"

#include <memory>
#include <iostream>

enum class GameObjectTag : std::uint8_t
{
    None,
    Plant,
    Zombie,
    Projectile,
    Ambient,
    Sun,
    Coin,
    Count
};

class Collider;
class Scene;

class GameObject : public std::enable_shared_from_this<GameObject>
{
public:
    virtual ~GameObject() = default;

    template <typename T, typename... Args>
    static std::shared_ptr<T> Create(Args&&... args);

    virtual void Update() = 0;
    virtual void Render();

    virtual void OnCollisionEnter(GameObject* other)
    {
    }

    virtual void OnCollisionStay(GameObject* other)
    {
    }

    virtual void OnCollisionExit(GameObject* other)
    {
    }

    [[nodiscard]] GameObjectTag GetTag() const;
    void SetTag(GameObjectTag tag);

    [[nodiscard]] Transform& GetTransform();
    [[nodiscard]] const Transform& GetTransform() const;
    void SetTransform(Transform transform);

    [[nodiscard]] bool IsActive() const;
    void SetActive(bool active);

    [[nodiscard]] bool IsVisible() const;
    void SetVisible(bool visible);

    [[nodiscard]] Collider* GetCollider() const { return collider_.get(); }
    void SetCollider(std::unique_ptr<Collider> collider);
    template <typename T, typename... Args>
    T* AddCollider(Args&&... args);

    void QueueFree();

    [[nodiscard]] Scene* GetScene() const { return scene_; }

protected:
    explicit GameObject(GameObjectTag tag = GameObjectTag::None);

    Transform transform_;
    GameObjectTag tag_;
    bool isActive_ = true;
    bool isVisible_ = true;
    bool isQueuedForDeletion_ = false;
    std::unique_ptr<Collider> collider_;
    Scene* scene_ = nullptr;

private:
    friend class Scene;
};

template <typename T, typename... Args>
T* GameObject::AddCollider(Args&&... args)
{
    static_assert(std::is_base_of_v<Collider, T>, "T must derive from Collider");
    auto collider = std::make_unique<T>(this, std::forward<Args>(args)...);
    T* ptr = collider.get();
    collider_ = std::move(collider);
    return ptr;
}

template <typename T, typename... Args>
std::shared_ptr<T> GameObject::Create(Args&&... args)
{
    static_assert(std::is_base_of_v<GameObject, T>, "T must derive from GameObject");
    struct EnableMakeShared : T
    {
        explicit EnableMakeShared(Args&&... args) : T(std::forward<Args>(args)...)
        {
        }
    };
    return std::make_shared<EnableMakeShared>(std::forward<Args>(args)...);
}
