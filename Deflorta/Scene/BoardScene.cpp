#include "BoardScene.hpp"

#include <algorithm>

#include "../Utils.hpp"
#include "../Base/Discord.hpp"
#include "../Object/Plant/SunFlower.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

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

    Transform tempCoverTransform{.x = 0, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
    Transform tempPoleTransform{.x = 0, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
    Transform tempTreeTransform{.x = 0, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};

    switch (settings_.backgroundType)
    {
    case BackgroundType::Day:
        loadGroup = "DelayLoad_Background1";
        bgName = "IMAGE_BACKGROUND1";
        coverName = "IMAGE_BACKGROUND1_COVER";
        hasCover = true;
        bushesNight = false;
        tempCoverTransform = Transform{.x = 920, .y = 580, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        break;
    case BackgroundType::Night:
        loadGroup = "DelayLoad_Background2";
        bgName = "IMAGE_BACKGROUND2";
        coverName = "IMAGE_BACKGROUND2_COVER";
        hasCover = true;
        bushesNight = true;
        tempCoverTransform = Transform{.x = 920, .y = 580, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        break;
    case BackgroundType::Pool:
        loadGroup = "DelayLoad_Background3";
        bgName = "IMAGE_BACKGROUND3";
        coverName = "IMAGE_BACKGROUND3_COVER";
        hasCover = true;
        bushesNight = false;
        tempCoverTransform = Transform{.x = 925, .y = 635, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        break;
    case BackgroundType::PoolNight:
        loadGroup = "DelayLoad_Background4";
        bgName = "IMAGE_BACKGROUND4";
        coverName = "IMAGE_BACKGROUND4_COVER";
        hasCover = true;
        bushesNight = true;
        tempCoverTransform = Transform{.x = 925, .y = 635, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        break;
    case BackgroundType::Roof:
        loadGroup = "DelayLoad_Background5";
        bgName = "IMAGE_BACKGROUND5";
        hasPole = true;
        hasTree = true;
        tempPoleTransform = Transform{.x = 870, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        tempTreeTransform = Transform{.x = 930, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        break;
    case BackgroundType::RoofNight:
        loadGroup = "DelayLoad_Background6";
        bgName = "IMAGE_BACKGROUND6";
        hasPole = true;
        hasTree = true;
        tempPoleTransform = Transform{.x = 870, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
        tempTreeTransform = Transform{.x = 930, .y = 0, .rotation = 0.0f, .scaleX = 1.0f, .scaleY = 1.0f};
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
        bushes_ = std::make_unique<Bush>(rows, bushesNight);
    }

    backgroundTransform_ = Transform{
        .x = -220,
        .y = 0,
        .rotation = 0.0f,
        .scaleX = 1.0f,
        .scaleY = 1.0f
    };

    if (settings_.hasFog)
    {
        fog_ = std::make_unique<Fog>(rows, settings_.fogColumns);
    }

    plants_.reserve(Utils::NextPowerOf2(rows * 9));

    auto [x1, y1] = GridToPosition(0, 0);
    auto testPlant = std::make_shared<SunFlower>(x1, y1);
    plants_.emplace_back(testPlant);

    auto [x2, y2] = GridToPosition(2, 4);
    auto testPlant2 = std::make_shared<SunFlower>(x2, y2);
    plants_.emplace_back(testPlant2);

    auto [x3, y3] = GridToPosition(4, 8);
    auto testPlant3 = std::make_shared<SunFlower>(x3, y3);
    plants_.emplace_back(testPlant3);
}

void BoardScene::OnEnter()
{
    const auto description = "Play level: " + settings_.levelName;
    Discord::SetPresence("Game", description);
}

void BoardScene::Update()
{
    for (const auto& plant : plants_)
        plant->Update();
    if (bushes_)
        bushes_->Update();
    if (fog_)
        fog_->Update();
}

void BoardScene::Render()
{
    Renderer::EnqueueImage(background_, backgroundTransform_);
    for (const auto& plant : plants_)
        plant->Draw();
    if (bushes_)
        bushes_->Draw();
    if (cover_)
        Renderer::EnqueueImage(cover_, coverTransform_);
    if (fog_)
        fog_->Render();
    if (tree_)
        Renderer::EnqueueImage(tree_, treeTransform_);
    if (pole_)
        Renderer::EnqueueImage(pole_, poleTransform_);
}

std::pair<float, float> BoardScene::GridToPosition(int row, int column)
{
    float x = kPlantBaseX + kPlantCellWidth * static_cast<float>(column);
    float y = kPlantBaseY + kPlantCellHeight * static_cast<float>(row);
    return {x, y};
}

std::pair<int, int> BoardScene::PositionToGrid(float x, float y)
{
    const float colF = (x - kPlantBaseX) / kPlantCellWidth;
    const float rowF = (y - kPlantBaseY) / kPlantCellHeight;

    int col = static_cast<int>(std::round(colF));
    int row = static_cast<int>(std::round(rowF));

    col = std::max(col, 0);
    row = std::max(row, 0);

    return {row, col};
}
