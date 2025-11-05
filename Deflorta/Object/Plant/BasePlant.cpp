#include "BasePlant.hpp"

#include "../../Scene/BoardScene.hpp"

BasePlant::BasePlant(PlantType type, PlantPos pos)
    : GameObject(GameObjectTag::Plant), plantType(type), plantPos(pos)
{
    auto collider = std::make_unique<BoxCollider>(this, 65.0f, 75.0f);
    collider->SetOffset({42.0f, 45.0f});
    SetCollider(std::move(collider));
}

void BasePlant::Render()
{
    if (sprite)
        sprite->Draw();
}

void BasePlant::Update()
{
    if (sprite)
    {
        sprite->SetPosition(transform_.position);
        sprite->Update();
    }
}

PlantType BasePlant::GetPlantType() const
{
    return plantType;
}

PlantPos BasePlant::GetPlantPos() const
{
    return plantPos;
}

void BasePlant::SetGridPosition(int row, int column, BackgroundType bgType)
{
    gridRow = row;
    gridColumn = column;
    const auto pos = BoardScene::GridToPosition(gridRow, gridColumn, bgType);
    transform_.position = pos;
}

int BasePlant::GetGridRow() const
{
    return gridRow;
}

int BasePlant::GetGridColumn() const
{
    return gridColumn;
}
