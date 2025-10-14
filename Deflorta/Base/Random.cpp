#include "Random.hpp"

#include <chrono>
#include <thread>

std::mt19937& Random::Engine()
{
    thread_local std::mt19937 rng = InitEngine();
    return rng;
}

int Random::UniformInt(const int min, const int max)
{
    std::uniform_int_distribution dist(min, max);
    return dist(Engine());
}

float Random::UniformFloat(const float min, const float max)
{
    std::uniform_real_distribution dist(min, max);
    return dist(Engine());
}

double Random::UniformDouble(const double min, const double max)
{
    std::uniform_real_distribution dist(min, max);
    return dist(Engine());
}

std::mt19937 Random::InitEngine()
{
    std::random_device rd;
    const auto seed = static_cast<unsigned>(
        rd() ^
        std::chrono::high_resolution_clock::now().time_since_epoch().count() ^
        std::hash<std::thread::id>{}(std::this_thread::get_id()));
    return std::mt19937(seed);
}
