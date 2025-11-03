#pragma once

#include <memory>

#include "../GameObject.hpp"
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

class BasePlant : public GameObject
{
public:
    BasePlant(PlantType type, PlantPos pos);

    void Update() override;
    void Render() override;

    [[nodiscard]] PlantType GetPlantType() const;
    [[nodiscard]] PlantPos GetPlantPos() const;

    void SetGridPosition(int row, int column, BackgroundType bgType);
    [[nodiscard]] int GetGridRow() const;
    [[nodiscard]] int GetGridColumn() const;

protected:
    std::unique_ptr<Reanimator> sprite;

    PlantType plantType;
    PlantPos plantPos;

    int gridRow = -1;
    int gridColumn = -1;
};
