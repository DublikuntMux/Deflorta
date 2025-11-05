#pragma once

#include <glm/mat3x3.hpp>

using Matrix = glm::mat3;

namespace MatrixHelper
{
    Matrix Identity();
    Matrix Translation(float x, float y);
    Matrix Scale(float sx, float sy);
    Matrix Rotation(float angleInDegrees);
    Matrix CreateMatrix(float m11, float m12, float m21, float m22, float dx, float dy);
}
