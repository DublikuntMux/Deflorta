#include "CollisionMatrix.hpp"

CollisionMatrix::CollisionMatrix()
{
    for (auto& row : matrix_)
    {
        row.fill(false);
    }

    EnableCollision(GameObjectTag::Projectile, GameObjectTag::Zombie);
    EnableCollision(GameObjectTag::Zombie, GameObjectTag::Plant);
    EnableCollision(GameObjectTag::Plant, GameObjectTag::Projectile);
}

size_t CollisionMatrix::TagToIndex(GameObjectTag tag)
{
    return static_cast<size_t>(tag);
}

bool CollisionMatrix::CanCollide(GameObjectTag tag1, GameObjectTag tag2) const
{
    const size_t index1 = TagToIndex(tag1);
    const size_t index2 = TagToIndex(tag2);

    if (index1 >= MAX_TAGS || index2 >= MAX_TAGS)
        return false;

    return matrix_[index1][index2];
}

void CollisionMatrix::SetCollision(GameObjectTag tag1, GameObjectTag tag2, bool canCollide)
{
    const size_t index1 = TagToIndex(tag1);
    const size_t index2 = TagToIndex(tag2);

    if (index1 < MAX_TAGS && index2 < MAX_TAGS)
    {
        matrix_[index1][index2] = canCollide;
    }
}

void CollisionMatrix::EnableCollision(GameObjectTag tag1, GameObjectTag tag2)
{
    SetCollision(tag1, tag2, true);
}

void CollisionMatrix::DisableCollision(GameObjectTag tag1, GameObjectTag tag2)
{
    SetCollision(tag1, tag2, false);
}
