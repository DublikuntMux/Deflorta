#pragma once

#include <glm/mat3x3.hpp>

namespace MatrixHelper
{
    glm::mat3 Identity();
    glm::mat3 Translation(glm::vec2 vec);
    glm::mat3 Scale(glm::vec2 scale);
    glm::mat3 Rotation(float angleInDegrees);
    glm::mat3 CreateMatrix(float m11, float m12, float m21, float m22, float dx, float dy);
}
