#include "Fog.hpp"

#include "../Base/Random.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

Fog::Fog(int rowCount, int maxColumns)
    : rowCount_(rowCount), maxColumns_(maxColumns)
{
    ResourceManager::LoadGroup("DelayLoad_Fog");
    GenerateFogPieces();
}

void Fog::Update() const
{
    for (auto& piece : fogPieces_)
    {
        if (piece.tween && piece.tween->IsActive())
        {
            piece.tween->Update();
        }
    }
}

void Fog::Render() const
{
    for (const auto& piece : fogPieces_)
    {
        if (piece.texture)
        {
            Renderer::EnqueueImage(piece.texture, piece.transform);
        }
    }
}

void Fog::MoveFog(float targetX, float duration)
{
    for (auto& piece : fogPieces_)
    {
        const float startX = piece.transform.x;
        
        piece.tween = std::make_unique<Tween>(
            std::vector{
                TweenProperty{
                    .start = startX,
                    .end = targetX + piece.col * 80.0f,
                    .setter = [&piece](float value) { piece.transform.x = value; },
                    .mode = TweenMode::EaseInOut
                }
            },
            duration
        );
        
        piece.tween->Start();
    }
}

ID2D1Bitmap* Fog::GetRandomFogTexture()
{
    const int fogIndex = Random::UniformInt(0, 7);
    const std::string fogId = "IMAGE_FOG_" + std::to_string(fogIndex);
    
    return ResourceManager::GetImage(fogId);
}

void Fog::GenerateFogPieces()
{
    fogPieces_.clear();
    fogPieces_.reserve(rowCount_ * maxColumns_);

    for (int row = 0; row < rowCount_; ++row)
    {
        for (int col = 0; col < maxColumns_; ++col)
        {
            FogPiece piece;
            piece.texture = GetRandomFogTexture();
            piece.row = row;
            piece.col = col;
            
            piece.transform.x = 870.0f - static_cast<float>(col) * 80.0f + Random::UniformFloat(-10.0f, 10.0f);
            piece.transform.y = 85.0f + static_cast<float>(row) * 90.0f + Random::UniformFloat(-10.0f, 10.0f);
            piece.transform.rotation = 0.0f;
            piece.transform.scaleX = 1.0f + Random::UniformFloat(-0.1f, 0.1f);
            piece.transform.scaleY = 1.0f + Random::UniformFloat(-0.1f, 0.1f);
            
            fogPieces_.push_back(std::move(piece));
        }
    }
}
