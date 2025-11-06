#include "Fog.hpp"

#include "../Base/Random.hpp"
#include "../Render/Layer.hpp"
#include "../Render/Renderer.hpp"
#include "../Resource/ResourceManager.hpp"

Fog::Fog(int rowCount, int maxColumns)
    : GameObject(GameObjectTag::Ambient), rowCount_(rowCount), maxColumns_(maxColumns)
{
    ResourceManager::LoadGroup("DelayLoad_Fog");
    GenerateFogPieces();
}

void Fog::Update()
{
    for (const auto& piece : fogPieces_)
    {
        if (piece.tween && piece.tween->IsActive())
        {
            piece.tween->Update();
        }
    }
}

void Fog::Render()
{
    for (const auto& piece : fogPieces_)
    {
        if (piece.texture)
        {
            Renderer::EnqueueImage(piece.texture, piece.transform, 1, static_cast<int>(RenderLayer::Fog));
        }
    }
}

void Fog::MoveFog(float targetX, float duration)
{
    for (auto& piece : fogPieces_)
    {
        const float startX = piece.transform.position.x;

        piece.tween = std::make_unique<Tween>(
            std::vector{
                TweenProperty{
                    .start = startX,
                    .end = targetX + piece.col * 80.0f,
                    .setter = [&piece](float value) { piece.transform.position.x = value; },
                    .mode = TweenMode::EaseInOut
                }
            },
            duration
        );

        piece.tween->Start();
    }
}

std::shared_ptr<ITexture> Fog::GetRandomFogTexture()
{
    const int fogIndex = Random::UniformInt(0, 7);
    const std::string fogId = "IMAGE_FOG_" + std::to_string(fogIndex);

    return ResourceManager::GetImage(fogId);
}

void Fog::GenerateFogPieces()
{
    fogPieces_.clear();
    const int totalRows = rowCount_ + 2;
    fogPieces_.reserve(totalRows * maxColumns_);

    for (int row = -1; row <= rowCount_; ++row)
    {
        for (int col = 0; col < maxColumns_; ++col)
        {
            FogPiece piece;
            piece.texture = GetRandomFogTexture();
            piece.row = row;
            piece.col = col;

            piece.transform.position.x = 870.0f - static_cast<float>(col) * 80.0f + Random::UniformFloat(-10.0f, 10.0f);
            piece.transform.position.y = 85.0f + static_cast<float>(row) * 90.0f + Random::UniformFloat(-10.0f, 10.0f);
            piece.transform.rotation = 0.0f;
            piece.transform.scale.x = 1.0f + Random::UniformFloat(-0.1f, 0.1f);
            piece.transform.scale.y = 1.0f + Random::UniformFloat(-0.1f, 0.1f);

            fogPieces_.push_back(std::move(piece));
        }
    }
}
