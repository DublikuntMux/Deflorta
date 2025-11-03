#include "BasePlant.hpp"

#include "../../Scene/BoardScene.hpp"

BasePlant::BasePlant(PlantType type, PlantPos pos)
    : GameObject(GameObjectTag::Plant), plantType(type), plantPos(pos)
{
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
        sprite->SetPosition(transform_.x, transform_.y);
        sprite->OverrideScale(transform_.scaleX, transform_.scaleY);
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
    const auto [x, y] = BoardScene::GridToPosition(gridRow, gridColumn, bgType);
    transform_.x = x;
    transform_.y = y;
}

int BasePlant::GetGridRow() const
{
    return gridRow;
}

int BasePlant::GetGridColumn() const
{
    return gridColumn;
}
