#pragma once

#include <glm/vec4.hpp>

struct Color
{
    glm::vec4 value = {1.0f, 1.0f, 1.0f, 1.0f};

    Color() = default;
    Color(float red, float green, float blue, float alpha = 1.0f);
    Color(const glm::vec4& color);

    float GetR() const { return value.r; }
    float GetG() const { return value.g; }
    float GetB() const { return value.b; }
    float GetA() const { return value.a; }

    void SetR(float r) { value.r = r; }
    void SetG(float g) { value.g = g; }
    void SetB(float b) { value.b = b; }
    void SetA(float a) { value.a = a; }

    static Color White();
    static Color Black();
    static Color Red();
    static Color Green();
    static Color Blue();
};
