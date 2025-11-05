#pragma once

#include "../Object/GameObject.hpp"

#include <array>

class CollisionMatrix
{
public:
    CollisionMatrix();

    [[nodiscard]] bool CanCollide(GameObjectTag tag1, GameObjectTag tag2) const;
    void SetCollision(GameObjectTag tag1, GameObjectTag tag2, bool canCollide);
    void EnableCollision(GameObjectTag tag1, GameObjectTag tag2);
    void DisableCollision(GameObjectTag tag1, GameObjectTag tag2);

private:
    static constexpr size_t MAX_TAGS = 8;
    std::array<std::array<bool, MAX_TAGS>, MAX_TAGS> matrix_;

    [[nodiscard]] static size_t TagToIndex(GameObjectTag tag);
};
