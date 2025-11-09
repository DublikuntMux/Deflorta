#include "BoardScene.hpp"

#include "../Utils.hpp"
#include "../Base/Discord.hpp"
#include "../Object/Plant/SunFlower.hpp"
#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

#include <algorithm>

BoardScene::BoardScene(BoardSettings settings) : settings_(std::move(settings))
{
    const int rows = settings_.backgroundType == BackgroundType::Pool ||
                     settings_.backgroundType == BackgroundType::PoolNight
                         ? 6
                         : 5;

    std::string loadGroup;
    std::string bgName;
    std::string coverName;
    bool hasCover = false;
    bool bushesNight = false;
    bool hasPole = false;
    bool hasTree = false;

    Transform tempCoverTransform;
    Transform tempPoleTransform;
    Transform tempTreeTransform;

    switch (settings_.backgroundType)
    {
    case BackgroundType::Day:
        loadGroup = "DelayLoad_Background1";
        bgName = "IMAGE_BACKGROUND1";
        coverName = "IMAGE_BACKGROUND1_COVER";
        hasCover = true;
        bushesNight = false;
        tempCoverTransform = Transform{.position = {920, 580}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    case BackgroundType::Night:
        loadGroup = "DelayLoad_Background2";
        bgName = "IMAGE_BACKGROUND2";
        coverName = "IMAGE_BACKGROUND2_COVER";
        hasCover = true;
        bushesNight = true;
        tempCoverTransform = Transform{.position = {920, 580}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    case BackgroundType::Pool:
        loadGroup = "DelayLoad_Background3";
        bgName = "IMAGE_BACKGROUND3";
        coverName = "IMAGE_BACKGROUND3_COVER";
        hasCover = true;
        bushesNight = false;
        tempCoverTransform = Transform{.position = {925, 635}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    case BackgroundType::PoolNight:
        loadGroup = "DelayLoad_Background4";
        bgName = "IMAGE_BACKGROUND4";
        coverName = "IMAGE_BACKGROUND4_COVER";
        hasCover = true;
        bushesNight = true;
        tempCoverTransform = Transform{.position = {925, 635}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    case BackgroundType::Roof:
        loadGroup = "DelayLoad_Background5";
        bgName = "IMAGE_BACKGROUND5";
        hasPole = true;
        hasTree = true;
        tempPoleTransform = Transform{.position = {870, 0}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        tempTreeTransform = Transform{.position = {930, 0}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    case BackgroundType::RoofNight:
        loadGroup = "DelayLoad_Background6";
        bgName = "IMAGE_BACKGROUND6";
        hasPole = true;
        hasTree = true;
        tempPoleTransform = Transform{.position = {870, 0}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        tempTreeTransform = Transform{.position = {930, 0}, .scale = {1.0f, 1.0f}, .rotation = 0.0f};
        break;
    }

    if (!loadGroup.empty())
        ResourceManager::LoadGroup(loadGroup);

    if (!bgName.empty())
        background_ = ResourceManager::GetImage(bgName);

    if (hasCover && !coverName.empty())
    {
        cover_ = ResourceManager::GetImage(coverName);
        coverTransform_ = tempCoverTransform;
    }

    if (hasPole)
    {
        const std::string poleName = settings_.backgroundType == BackgroundType::Roof
                                         ? "IMAGE_BACKGROUND5_POLE"
                                         : "IMAGE_BACKGROUND6_POLE";
        pole_ = ResourceManager::GetImage(poleName);
        poleTransform_ = tempPoleTransform;
    }

    if (hasTree)
    {
        const std::string treeName = settings_.backgroundType == BackgroundType::Roof
                                         ? "IMAGE_BACKGROUND5_TREES"
                                         : "IMAGE_BACKGROUND6_TREES";
        tree_ = ResourceManager::GetImage(treeName);
        treeTransform_ = tempTreeTransform;
    }

    if (!hasPole)
    {
        bush_ = GameObject::Create<Bush>(rows, bushesNight);
        AddGameObject(bush_);
    }

    backgroundTransform_ = Transform{
        .position = {-220, 0},
        .scale = {1.0f, 1.0f},
        .rotation = 0.0f
    };

    if (settings_.hasFog)
    {
        fog_ = GameObject::Create<Fog>(rows, settings_.fogColumns);
        AddGameObject(fog_);
    }

    seedBank_ = GameObject::Create<SeedBank>();
    AddGameObject(seedBank_);

    plants_.reserve(Utils::NextPowerOf2(rows * 9));

    for (int row = 0; row < 5; ++row)
    {
        for (int col = 0; col < 9; ++col)
        {
            auto testPlant = GameObject::Create<SunFlower>();
            PlantAt(row, col, std::move(testPlant));
        }
    }
}

void BoardScene::OnEnter()
{
    const auto description = "Play level: " + settings_.levelName;
    Discord::SetPresence("Game", description);
}

void BoardScene::Update()
{
    Scene::Update();
}

void BoardScene::Render()
{
    Scene::Render();

    Renderer::EnqueueImage(background_, backgroundTransform_, 1, static_cast<int>(RenderLayer::Background));
    if (cover_)
        Renderer::EnqueueImage(cover_, coverTransform_, 1, static_cast<int>(RenderLayer::Foreground));
    if (tree_)
        Renderer::EnqueueImage(tree_, treeTransform_, 1, static_cast<int>(RenderLayer::Foreground));
    if (pole_)
        Renderer::EnqueueImage(pole_, poleTransform_, 1, static_cast<int>(RenderLayer::Foreground));
}

glm::vec2 BoardScene::GridToPosition(int row, int column, BackgroundType bgType)
{
    float x = kPlantBaseX + kPlantCellWidth * static_cast<float>(column);
    float y;

    if (bgType == BackgroundType::Roof ||
        bgType == BackgroundType::RoofNight)
    {
        if (column >= 5)
        {
            y = kPlantBaseYRoofFlat + kPlantCellHeightRoof * static_cast<float>(row);
        }
        else
        {
            y = kPlantBaseYRoof - static_cast<float>(column) * 20.0f + kPlantCellHeightRoof * static_cast<float>(row);
        }
    }
    else
    {
        y = kPlantBaseY + kPlantCellHeight * static_cast<float>(row);
    }

    return {x, y};
}

glm::ivec2 BoardScene::PositionToGrid(const glm::vec2& position, BackgroundType bgType)
{
    const float colF = (position.x - kPlantBaseX) / kPlantCellWidth;
    int col = static_cast<int>(std::round(colF));
    col = std::clamp(col, 0, 8);

    float rowF;
    if (bgType == BackgroundType::Roof ||
        bgType == BackgroundType::RoofNight)
    {
        if (col >= 5)
        {
            rowF = (position.y - kPlantBaseYRoofFlat) / kPlantCellHeightRoof;
        }
        else
        {
            rowF = (position.y - kPlantBaseYRoof + static_cast<float>(col) * 20.0f) / kPlantCellHeightRoof;
        }
    }
    else
    {
        rowF = (position.y - kPlantBaseY) / kPlantCellHeight;
    }

    int row = static_cast<int>(std::round(rowF));
    row = std::clamp(row, 0, 5);

    return {row, col};
}

bool BoardScene::CanPlantAt(int row, int column, PlantLayer layer) const
{
    const int maxRows = settings_.backgroundType == BackgroundType::Pool ||
                        settings_.backgroundType == BackgroundType::PoolNight
                            ? 6
                            : 5;
    constexpr int maxColumns = 9;

    if (row < 0 || row >= maxRows || column < 0 || column >= maxColumns)
        return false;

    return std::ranges::all_of(plants_, [&](const auto& plant)
    {
        return !(plant->GetGridRow() == row &&
            plant->GetGridColumn() == column &&
            plant->GetPlantLayer() == layer);
    });
}

bool BoardScene::PlantAt(int row, int column, std::shared_ptr<BasePlant> plant)
{
    if (!plant)
        return false;

    if (!CanPlantAt(row, column, plant->GetPlantLayer()))
        return false;

    plant->SetGridPosition(row, column, settings_.backgroundType);
    plants_.push_back(plant);
    AddGameObject(std::move(plant));

    return true;
}
