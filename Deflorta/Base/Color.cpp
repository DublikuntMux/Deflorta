#include "Color.hpp"

Color::Color(float red, float green, float blue, float alpha) : value(red, green, blue, alpha)
{
}

Color::Color(const glm::vec4& color) : value(color)
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
