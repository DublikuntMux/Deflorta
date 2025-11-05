#include "Color.hpp"

Color::Color(float red, float green, float blue, float alpha) : r(red), g(green), b(blue), a(alpha)
{
}

Color Color::White()
{
    return {1.0f, 1.0f, 1.0f, 1.0f};
}

Color Color::Black()
{
    return {0.0f, 0.0f, 0.0f, 1.0f};
}

Color Color::Red()
{
    return {1.0f, 0.0f, 0.0f, 1.0f};
}

Color Color::Green()
{
    return {0.0f, 1.0f, 0.0f, 1.0f};
}

Color Color::Blue()
{
    return {0.0f, 0.0f, 1.0f, 1.0f};
}
