#pragma once

#include <chrono>

class Time
{
public:
    void tick();
    [[nodiscard]] float deltaTime() const noexcept;

private:
    std::chrono::steady_clock::time_point lastTime_ = std::chrono::steady_clock::now();
    float deltaTime_ = 0.0f;
};
