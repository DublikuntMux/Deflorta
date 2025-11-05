#pragma once

#include <memory>
#include <vector>

#include "GameObject.hpp"
#include "../Render/Reanimator.hpp"

class Bush : public GameObject
{
public:
    Bush(int rowCount, bool isNightMode);

    void Update() override;
    void Render() override;

    void Rustle(int row) const;

private:
    std::vector<std::shared_ptr<Reanimator>> bushAnimations_;
    static std::vector<glm::vec2> GenerateBushPositions(int rowCount);
};
