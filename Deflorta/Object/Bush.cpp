#include "Bush.hpp"

#include "../Base/Random.hpp"

Bush::Bush(int rowCount, bool isNightMode)
{
    auto bush1 = isNightMode
                     ? ReanimationLoader::LoadFromFile("resources/reanim/Night_bush1.xml")
                     : ReanimationLoader::LoadFromFile("resources/reanim/bush1.xml");
    auto bush2 = isNightMode
                     ? ReanimationLoader::LoadFromFile("resources/reanim/Night_bush2.xml")
                     : ReanimationLoader::LoadFromFile("resources/reanim/bush2.xml");
    auto bush3 = isNightMode
                     ? ReanimationLoader::LoadFromFile("resources/reanim/Night_bush3.xml")
                     : ReanimationLoader::LoadFromFile("resources/reanim/bush3.xml");
    if (!bush1.has_value() || !bush2.has_value() || !bush3.has_value())
        throw std::runtime_error("Failed to load selector bushes reanim");

    const auto positions = GenerateBushPositions(rowCount);
    bushAnimations_.reserve(rowCount);
    for (int i = 0; i < rowCount; ++i)
    {
        const auto randBush = Random::UniformInt(1, 3);
        std::shared_ptr<Reanimator> bushAnim;
        if (randBush == 1)
            bushAnim = std::make_shared<Reanimator>(bush1.value());
        else if (randBush == 2)
            bushAnim = std::make_shared<Reanimator>(bush2.value());
        else
            bushAnim = std::make_shared<Reanimator>(bush3.value());
        bushAnim->SetPosition(static_cast<float>(positions[i].first), static_cast<float>(positions[i].second));
        bushAnim->PlayLayer("anim_rustle", ReanimLoopType::PlayOnceAndHold);
        bushAnimations_.push_back(bushAnim);
    }
}

void Bush::Rustle(int row) const
{
    bushAnimations_[row]->PlayLayer("anim_rustle", ReanimLoopType::PlayOnceAndHold);
}

void Bush::Update() const
{
    for (const auto& bush : bushAnimations_)
    {
        bush->Update();
    }
}

void Bush::Draw() const
{
    for (const auto& bush : bushAnimations_)
    {
        bush->Draw();
    }
}

std::vector<std::pair<int, int>> Bush::GenerateBushPositions(int rowCount)
{
    std::vector<std::pair<int, int>> positions;
    positions.reserve(rowCount);

    for (int i = 0; i < rowCount; ++i)
    {
        int x = 970 + Random::UniformInt(-10, 10);
        int y = 80 + i * 90 + Random::UniformInt(-5, 5);
        positions.emplace_back(x, y);
    }

    return positions;
}
