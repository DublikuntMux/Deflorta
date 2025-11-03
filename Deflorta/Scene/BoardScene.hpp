#pragma once

#include <string>
#include <utility>

#include <d2d1.h>

#include "Scene.hpp"
#include "../Base/Transform.hpp"
#include "../Object/Bush.hpp"
#include "../Object/Fog.hpp"
#include "../Object/Plant/BasePlant.hpp"

enum class BackgroundType: std::uint8_t
{
    Day,
    Night,
    Pool,
    PoolNight,
    Roof,
    RoofNight,
};

struct BoardSettings
{
    std::string levelName;

    BackgroundType backgroundType;
    bool hasFog;
    int fogColumns;
};

static constexpr float kPlantBaseX = 270.0f;
static constexpr float kPlantBaseY = 150.0f;
static constexpr float kPlantCellWidth = 81.0f;
static constexpr float kPlantCellHeight = 97.0f;

static constexpr float kPlantBaseYRoof = kPlantBaseY + 87.0f;
static constexpr float kPlantBaseYRoofFlat = kPlantBaseY - 10.0f;
static constexpr float kPlantCellHeightRoof = 85.0f;

class BoardScene final : public Scene
{
public:
    BoardScene(BoardSettings settings);

    void OnEnter() override;
    void Update() override;
    void Render() override;

    static std::pair<float, float> GridToPosition(int row, int column, BackgroundType bgType);
    static std::pair<int, int> PositionToGrid(float x, float y, BackgroundType bgType);

private:
    BoardSettings settings_;

    ID2D1Bitmap* background_ = nullptr;
    ID2D1Bitmap* cover_ = nullptr;
    ID2D1Bitmap* pole_ = nullptr;
    ID2D1Bitmap* tree_ = nullptr;

    Transform backgroundTransform_;
    Transform coverTransform_;
    Transform poleTransform_;
    Transform treeTransform_;

    std::shared_ptr<Bush> bush_;
    std::shared_ptr<Fog> fog_;

    std::vector<std::shared_ptr<BasePlant>> plants_;

    bool CanPlantAt(int row, int column, PlantPos pos) const;
    bool PlantAt(int row, int column, const std::shared_ptr<BasePlant>& plant);
};
