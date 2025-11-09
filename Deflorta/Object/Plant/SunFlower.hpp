#pragma once

#include "BasePlant.hpp"

#include "../../Base/Timer.hpp"

class SunFlower : public BasePlant
{
public:
    SunFlower();

    void Update() override;

private:
    void ProduceSun();

    std::unique_ptr<Timer> timer_;
};
