#pragma once

#include <string>

#include "../Base/Transform.hpp"

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

class GameObject
{
public:
    explicit GameObject(GameObjectTag tag = GameObjectTag::None);
    virtual ~GameObject() = default;

    virtual void Update() = 0;
    virtual void Render() = 0;

    [[nodiscard]] GameObjectTag GetTag() const;
    void SetTag(GameObjectTag tag);

    [[nodiscard]] Transform& GetTransform();
    [[nodiscard]] const Transform& GetTransform() const;
    void SetTransform(const Transform& transform);

    [[nodiscard]] bool IsActive() const;
    void SetActive(bool active);

protected:
    Transform transform_;
    GameObjectTag tag_;
    bool isActive_ = true;
};
