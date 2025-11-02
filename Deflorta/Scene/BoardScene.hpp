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
static constexpr float kPlantCellWidth = 81.5f;
static constexpr float kPlantCellHeight = 97.0f;

class BoardScene final : public Scene
{
public:
    BoardScene(BoardSettings settings);

    void OnEnter() override;
    void Update() override;
    void Render() override;

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

    std::unique_ptr<Bush> bushes_;
    std::unique_ptr<Fog> fog_;

    std::vector<std::shared_ptr<BasePlant>> plants_;

    static std::pair<float, float> GridToPosition(int row, int column);
    static std::pair<int, int> PositionToGrid(float x, float y);
};
