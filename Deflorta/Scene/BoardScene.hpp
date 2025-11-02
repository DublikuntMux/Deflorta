#pragma once

#include <d2d1.h>
#include <string>

#include "Scene.hpp"
#include "../Base/Transform.hpp"
#include "../Object/Bush.hpp"

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
};

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
};
