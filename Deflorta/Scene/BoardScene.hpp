#pragma once

#include "Scene.hpp"
#include "../Base/Transform.hpp"
#include "../Object/Bush.hpp"
#include "../Object/Fog.hpp"
#include "../Object/Plant/BasePlant.hpp"
#include "../Object/SeedBank.hpp"
#include "../Base/Timer.hpp"

#include <string>

enum class BackgroundType: std::uint8_t
{
    Day,
    Night,
    Pool,
    NightPool,
    Roof,
    NightRoof,
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

static constexpr float kPlantCellHeightPool = 87.0f;

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

    static glm::vec2 GridToPosition(int row, int column, BackgroundType bgType);
    static glm::ivec2 PositionToGrid(const glm::vec2& position, BackgroundType bgType);

    [[nodiscard]] std::shared_ptr<SeedBank> GetSeedBank() const;

private:
    BoardSettings settings_;

    std::shared_ptr<ITexture> background_;
    std::shared_ptr<ITexture> cover_;
    std::shared_ptr<ITexture> pole_;
    std::shared_ptr<ITexture> tree_;

    Transform backgroundTransform_;
    Transform coverTransform_;
    Transform poleTransform_;
    Transform treeTransform_;

    std::shared_ptr<Bush> bush_;
    std::shared_ptr<Fog> fog_;
    std::shared_ptr<SeedBank> seedBank_;

    std::vector<std::shared_ptr<BasePlant>> plants_;

    std::unique_ptr<Timer> skySunTimer_;

    bool CanPlantAt(int row, int column, PlantLayer layer) const;
    bool PlantAt(int row, int column, std::shared_ptr<BasePlant> plant);

    void SpawnSkySun();
};
