#pragma once

#include <memory>

#include "PlantType.hpp"
#include "../../Render/Reanimator.hpp"

class BasePlant
{
public:
    BasePlant(PlantType type, float posX, float posY);
    virtual ~BasePlant();

    virtual void Draw();
    virtual void Update();

    [[nodiscard]] PlantType GetType() const;

protected:
    std::unique_ptr<Reanimator> sprite;
    Transform transform;

    PlantType plantType;
};
