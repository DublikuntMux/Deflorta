#include "BasePlant.hpp"

BasePlant::BasePlant(PlantType type, float posX, float posY)
{
    plantType = type;
    transform = Transform{
        .x = posX,
        .y = posY,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
    };
}

BasePlant::~BasePlant() = default;

void BasePlant::Draw()
{
    if (sprite)
        sprite->Draw();
}

void BasePlant::Update()
{
    if (sprite)
    {
        sprite->SetPosition(transform.x, transform.y);
        sprite->OverrideScale(transform.scaleX, transform.scaleY);
        sprite->Update();
    }
}

PlantType BasePlant::GetType() const
{
    return plantType;
}
