#pragma once

#include "GameObject.hpp"
#include "../Base/Transform.hpp"
#include "../Base/Tween.hpp"
#include "../Render/IRenderBackend.hpp"

#include <memory>
#include <vector>

struct FogPiece
{
    std::shared_ptr<ITexture> texture;
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

    std::shared_ptr<ITexture> GetRandomFogTexture();
    void GenerateFogPieces();
};
