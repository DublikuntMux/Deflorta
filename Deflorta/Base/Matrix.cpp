#include "Matrix.hpp"

#include <glm/gtc/matrix_transform.hpp>

namespace MatrixHelper
{
    glm::mat3 Identity()
    {
        return {1.0f};
    }

    glm::mat3 Translation(glm::vec2 vec)
    {
        glm::mat3 mat(1.0f);
        mat[2][0] = vec.x;
        mat[2][1] = vec.y;
        return mat;
    }

    glm::mat3 Scale(glm::vec2 scale)
    {
        glm::mat3 mat(1.0f);
        mat[0][0] = scale.x;
        mat[1][1] = scale.y;
        return mat;
    }

    glm::mat3 Rotation(float angleInDegrees)
    {
        const float rad = glm::radians(angleInDegrees);
        const float c = std::cos(rad);
        const float s = std::sin(rad);
        glm::mat3 mat(1.0f);
        mat[0][0] = c;
        mat[0][1] = s;
        mat[1][0] = -s;
        mat[1][1] = c;
        return mat;
    }

    glm::mat3 CreateMatrix(float m11, float m12, float m21, float m22, float dx, float dy)
    {
        glm::mat3 mat(1.0f);
        mat[0][0] = m11;
        mat[0][1] = m12;
        mat[1][0] = m21;
        mat[1][1] = m22;
        mat[2][0] = dx;
        mat[2][1] = dy;
        return mat;
    }
}
