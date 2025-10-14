#pragma once

#include <random>

class Random
{
public:
    static std::mt19937& Engine();

    static int UniformInt(int min, int max);
    static float UniformFloat(float min, float max);
    static double UniformDouble(double min, double max);

private:
    static std::mt19937 InitEngine();
};
