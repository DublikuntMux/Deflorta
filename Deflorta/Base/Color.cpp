#include "Color.hpp"

Color::Color(float red, float green, float blue, float alpha) : value(red, green, blue, alpha)
{
}

Color::Color(const glm::vec4& color) : value(color)
{
}

const Color Color::White = Color(1.0f, 1.0f, 1.0f, 1.0f);
const Color Color::Black = Color(0.0f, 0.0f, 0.0f, 1.0f);
const Color Color::Red = Color(1.0f, 0.0f, 0.0f, 1.0f);
const Color Color::Green = Color(0.0f, 1.0f, 0.0f, 1.0f);
const Color Color::Blue = Color(0.0f, 0.0f, 1.0f, 1.0f);
