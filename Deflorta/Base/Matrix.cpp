#include "Matrix.hpp"

#include <glm/gtc/matrix_transform.hpp>

namespace MatrixHelper
{
    Matrix Identity()
    {
        return {1.0f};
    }

    Matrix Translation(float x, float y)
    {
        Matrix mat(1.0f);
        mat[2][0] = x;
        mat[2][1] = y;
        return mat;
    }

    Matrix Scale(float sx, float sy)
    {
        Matrix mat(1.0f);
        mat[0][0] = sx;
        mat[1][1] = sy;
        return mat;
    }

    Matrix Rotation(float angleInDegrees)
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

    Matrix CreateMatrix(float m11, float m12, float m21, float m22, float dx, float dy)
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
