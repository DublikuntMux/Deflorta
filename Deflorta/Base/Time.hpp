#pragma once

#include <chrono>

class Time
{
public:
    static void tick();

    [[nodiscard]] static float deltaTime() noexcept;
    [[nodiscard]] static double fps() noexcept;

private:
    static std::chrono::steady_clock::time_point lastTime_;
    static std::chrono::steady_clock::time_point lastSecTime_;
    static float deltaTime_;
    static double fps_;
    static uint64_t frameCount_;
};
