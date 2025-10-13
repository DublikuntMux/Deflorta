#pragma once

#include <random>

class Random {
public:
    static auto Engine() -> std::mt19937 &;

    static auto UniformInt(int min, int max) -> int;
    static auto UniformFloat(float min, float max) -> float;
    static auto UniformDouble(double min, double max) -> double;

private:
    static auto InitEngine() -> std::mt19937;
};
