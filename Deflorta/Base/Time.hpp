#pragma once

#include <chrono>

class Time
{
public:
    static void Tick();

    [[nodiscard]] static float GetDeltaTime() noexcept;
    [[nodiscard]] static double GetFps() noexcept;

private:
    static std::chrono::steady_clock::time_point lastTime_;
    static std::chrono::steady_clock::time_point lastSecTime_;
    static float deltaTime_;
    static double fps_;
    static uint64_t frameCount_;
};
