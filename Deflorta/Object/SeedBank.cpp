#include "SeedBank.hpp"

#include <algorithm>

#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

SeedBank::SeedBank()
{
    seedBankTexture_ = ResourceManager::GetImage("IMAGE_SEEDBANK");
    seedBankTransform_ = Transform{.position = {50.0f, 15.0f}, .scale = {1.2f, 1.2f}, .rotation = 0.0f};

    sunValueRect_ = Rect::FromXYWH(27.0f, 80.0f, 50.0f, 30.0f);
}

void SeedBank::Update()
{
}

void SeedBank::Render()
{
    Renderer::EnqueueImage(seedBankTexture_, seedBankTransform_, 1.0f, static_cast<int>(RenderLayer::UI));
    Renderer::EnqueueTextW(std::to_wstring(sunValue_), sunValueRect_, L"BrianneTod", 24.0f, Color::Black,
                           static_cast<int>(RenderLayer::UI), Justification::Center);
}

int SeedBank::GetSunValue() const
{
    return sunValue_;
}

void SeedBank::IncrementSunValue(int amount)
{
    sunValue_ += amount;
    sunValue_ = std::clamp(sunValue_, 0, 99995);
}

bool SeedBank::SpendSun(int amount)
{
    if (sunValue_ >= amount)
    {
        sunValue_ -= amount;
        return true;
    }
    return false;
}
