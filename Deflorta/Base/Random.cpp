#include "Random.hpp"

#include <chrono>
#include <thread>

auto Random::Engine() -> std::mt19937 & {
    thread_local std::mt19937 rng = InitEngine();
    return rng;
}

auto Random::UniformInt(const int min, const int max) -> int {
    std::uniform_int_distribution dist(min, max);
    return dist(Engine());
}

auto Random::UniformFloat(const float min, const float max) -> float {
    std::uniform_real_distribution dist(min, max);
    return dist(Engine());
}

auto Random::UniformDouble(const double min, const double max) -> double {
    std::uniform_real_distribution dist(min, max);
    return dist(Engine());
}

auto Random::InitEngine() -> std::mt19937 {
    std::random_device rd;
    const auto seed = static_cast<unsigned>(
        rd() ^
        std::chrono::high_resolution_clock::now().time_since_epoch().count() ^
        std::hash<std::thread::id>{}(std::this_thread::get_id()));
    return std::mt19937(seed);
}