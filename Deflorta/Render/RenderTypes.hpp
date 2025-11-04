#pragma once

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>

#include <cstdint>
#include <vector>

class TextureHandle;
class TextFormatHandle;

struct Color
{
    float r = 1.0f;
    float g = 1.0f;
    float b = 1.0f;
    float a = 1.0f;

    Color() = default;

    Color(float red, float green, float blue, float alpha = 1.0f)
        : r(red), g(green), b(blue), a(alpha)
    {
    }

    static Color White() { return {1.0f, 1.0f, 1.0f, 1.0f}; }
    static Color Black() { return {0.0f, 0.0f, 0.0f, 1.0f}; }
    static Color Red() { return {1.0f, 0.0f, 0.0f, 1.0f}; }
    static Color Green() { return {0.0f, 1.0f, 0.0f, 1.0f}; }
    static Color Blue() { return {0.0f, 0.0f, 1.0f, 1.0f}; }
    static Color CornflowerBlue() { return {0.392f, 0.584f, 0.929f, 1.0f}; }
};

struct Rect
{
    float left = 0.0f;
    float top = 0.0f;
    float right = 0.0f;
    float bottom = 0.0f;

    Rect() = default;

    Rect(float l, float t, float r, float b)
        : left(l), top(t), right(r), bottom(b)
    {
    }

    float Width() const { return right - left; }
    float Height() const { return bottom - top; }
};

using Point2F = glm::vec2;

struct Size2F
{
    float width = 0.0f;
    float height = 0.0f;

    Size2F() = default;

    Size2F(float w, float h) : width(w), height(h)
    {
    }
};

using Matrix = glm::mat3;

namespace MatrixHelper
{
    inline Matrix Identity()
    {
        return {1.0f};
    }

    inline Matrix Translation(float x, float y)
    {
        Matrix mat(1.0f);
        mat[2][0] = x;
        mat[2][1] = y;
        return mat;
    }

    inline Matrix Scale(float sx, float sy)
    {
        Matrix mat(1.0f);
        mat[0][0] = sx;
        mat[1][1] = sy;
        return mat;
    }

    inline Matrix Rotation(float angleInDegrees)
    {
        const float rad = glm::radians(angleInDegrees);
        const float c = std::cos(rad);
        const float s = std::sin(rad);
        Matrix mat(1.0f);
        mat[0][0] = c;
        mat[0][1] = s;
        mat[1][0] = -s;
        mat[1][1] = c;
        return mat;
    }

    inline Matrix CreateMatrix(float m11, float m12, float m21, float m22, float dx, float dy)
    {
        Matrix mat(1.0f);
        mat[0][0] = m11;
        mat[0][1] = m12;
        mat[1][0] = m21;
        mat[1][1] = m22;
        mat[2][0] = dx;
        mat[2][1] = dy;
        return mat;
    }
}

struct PixelData
{
    std::vector<uint8_t> pixels;
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t pitch = 0;
};
