#pragma once

#include <memory>

#include "../../Render/Reanimator.hpp"

enum class BackgroundType : std::uint8_t;

enum class PlantType: std::uint8_t
{
    PeaShooter,
    SunFlower
};

enum class PlantPos: std::uint8_t
{
    Top,
    Base,
    Cover,
    Under
};

class BasePlant
{
public:
    BasePlant(PlantType type, PlantPos pos);
    virtual ~BasePlant();

    virtual void Draw();
    virtual void Update();

    [[nodiscard]] PlantType GetPlantType() const;
    [[nodiscard]] PlantPos GetPlantPos() const;

    void SetGridPosition(int row, int column, BackgroundType bgType);
    [[nodiscard]] int GetGridRow() const;
    [[nodiscard]] int GetGridColumn() const;

protected:
    std::unique_ptr<Reanimator> sprite;
    Transform transform;

    PlantType plantType;
    PlantPos plantPos;

    int gridRow = -1;
    int gridColumn = -1;
};
