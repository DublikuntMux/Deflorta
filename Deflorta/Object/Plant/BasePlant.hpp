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

enum class PlantLayer: std::uint8_t
{
    Top,
    Base,
    Cover,
    Under
};

class BasePlant : public GameObject
{
public:
    BasePlant(PlantType type, PlantLayer layer);

    void Update() override;
    void Render() override;

    [[nodiscard]] PlantType GetPlantType() const;
    [[nodiscard]] PlantLayer GetPlantLayer() const;

    void SetGridPosition(int row, int column, BackgroundType bgType);
    [[nodiscard]] int GetGridRow() const;
    [[nodiscard]] int GetGridColumn() const;

protected:
    std::unique_ptr<Reanimator> sprite;

    PlantType plantType;
    PlantLayer plantLayer;

    int gridRow = -1;
    int gridColumn = -1;
};
