#pragma once

struct Color
{
    float r = 1.0f;
    float g = 1.0f;
    float b = 1.0f;
    float a = 1.0f;

    Color() = default;
    Color(float red, float green, float blue, float alpha = 1.0f);

    static Color White();
    static Color Black();
    static Color Red();
    static Color Green();
    static Color Blue();
};
