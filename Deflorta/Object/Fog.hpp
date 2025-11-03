#pragma once

#include <memory>
#include <vector>

#include <d2d1.h>

#include "GameObject.hpp"
#include "../Base/Transform.hpp"
#include "../Base/Tween.hpp"

struct FogPiece
{
    ID2D1Bitmap* texture = nullptr;
    Transform transform;
    std::unique_ptr<Tween> tween;
    int row = 0;
    int col = 0;
};

class Fog : public GameObject
{
public:
    Fog(int rowCount, int maxColumns);

    void Update() override;
    void Render() override;

    void MoveFog(float targetX, float duration);

private:
    std::vector<FogPiece> fogPieces_;
    int rowCount_;
    int maxColumns_;

    static ID2D1Bitmap* GetRandomFogTexture();
    void GenerateFogPieces();
};
