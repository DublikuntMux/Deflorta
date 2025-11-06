#pragma once

#include "ClickableObject.hpp"
#include "../../Render/Reanimator.hpp"

class SunObject final : public ClickableObject
{
public:
    SunObject();

    void Render() override;
    void Update() override;

    void OnMouseClick() override;

private:
    std::unique_ptr<Reanimator> sprite;
};
