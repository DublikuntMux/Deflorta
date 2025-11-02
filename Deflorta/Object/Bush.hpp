#pragma once

#include <memory>
#include <vector>
#include <utility>

#include "../Render/Reanimator.hpp"

class Bush
{
public:
    Bush(int rowCount, bool isNightMode);

    void Rustle(int row) const;
    void Update() const;
    void Draw() const;

private:
    std::vector<std::shared_ptr<Reanimator>> bushAnimations_;
    static std::vector<std::pair<int, int>> GenerateBushPositions(int rowCount);
};
