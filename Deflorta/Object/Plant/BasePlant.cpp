#include "BasePlant.hpp"

#include "../../Scene/BoardScene.hpp"

BasePlant::BasePlant(PlantType type, PlantPos pos) : plantType(type), plantPos(pos)
{
    transform = Transform{
        .x = 0.0f,
        .y = 0.0f,
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
    transform.x = x;
    transform.y = y;
}

int BasePlant::GetGridRow() const
{
    return gridRow;
}

int BasePlant::GetGridColumn() const
{
    return gridColumn;
}
