#include "Time.hpp"

void Time::tick()
{
    auto now = std::chrono::steady_clock::now();
    deltaTime_ = std::chrono::duration<float>(now - lastTime_).count();
    lastTime_ = now;
}

float Time::deltaTime() const noexcept
{
    return deltaTime_;
}
