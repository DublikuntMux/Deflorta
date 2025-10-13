#include "Time.hpp"

std::chrono::steady_clock::time_point Time::lastTime_ = std::chrono::steady_clock::now();
std::chrono::steady_clock::time_point Time::lastSecTime_ = std::chrono::steady_clock::now();
float Time::deltaTime_ = 0.0f;
double Time::fps_ = 0.0;
uint64_t Time::frameCount_ = 0;

void Time::tick()
{
    const auto now = std::chrono::steady_clock::now();
    deltaTime_ = std::chrono::duration<float>(now - lastTime_).count();

    ++frameCount_;
    const double elapsed = std::chrono::duration<double>(now - lastSecTime_).count();
    if (elapsed >= 1.0)
    {
        fps_ = static_cast<double>(frameCount_) / elapsed;
        frameCount_ = 0;
        lastSecTime_ = now;
    }

    lastTime_ = now;
}

float Time::deltaTime() noexcept
{
    return deltaTime_;
}

double Time::fps() noexcept
{
    return fps_;
}
