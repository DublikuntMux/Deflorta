#pragma once

#include "GameObject.hpp"
#include "../Render/IRenderBackend.hpp"

class SeedBank : public GameObject
{
public:
    SeedBank();

    void Update() override;
    void Render() override;

    int GetSunValue() const;
    void IncrementSunValue(int amount);
    bool SpendSun(int amount);

private:
    std::shared_ptr<ITexture> seedBankTexture_;
    Transform seedBankTransform_;

    Rect sunValueRect_;
    int sunValue_ = 0;
};
