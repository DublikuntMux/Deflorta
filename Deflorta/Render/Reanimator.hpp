#pragma once

#include "Layer.hpp"
#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Transform.hpp"

#include <string>
#include <vector>
#include <optional>

enum class ReanimLoopType: std::uint8_t
{
    Loop,
    PlayOnce,
    PlayOnceAndHold,
    LoopFullLastFrame,
    PlayOnceFullLastFrame,
    PlayOnceFullLastFrameAndHold
};

struct FrameTime
{
    int before = 0;
    int after = 0;
    float frac = 0.0f;
};

struct TrackInstance
{
    int renderGroup = static_cast<int>(RenderLayer::Default);
    float shakeOverride = 0.0f;
    float shakeX = 0.0f;
    float shakeY = 0.0f;
    std::optional<std::string> imageOverride;
    bool visible = true;
};

class Reanimator final
{
public:
    explicit Reanimator(const ReanimatorDefinition* def);

    void PlayLayer(const std::string& trackName,
                   ReanimLoopType loopType = ReanimLoopType::Loop,
                   float animRate = 0.0f,
                   float blendTime = 0.0f);

    void SetFramesForLayer(const std::string& trackName);
    [[nodiscard]] std::pair<int, int> GetFramesForLayer(const std::string& trackName) const;

    void Update();
    void Draw() const;

    void SetPosition(float x, float y);
    void OverrideScale(float sx, float sy);
    void OverrideLayerImage(const std::string& trackName, const std::string& image);
    void ClearLayerImageOverride(const std::string& trackName);
    void SetLayerVisible(const std::string& trackName, bool visible);
    void SetLayerZ(const std::string& trackName, int z);
    void SetAllLayersZ(int z);

    [[nodiscard]] bool IsDead() const { return dead_; }
    [[nodiscard]] bool IsFinished() const;

private:
    int FindTrackIndexByName(const std::string& trackName) const;

    [[nodiscard]] FrameTime GetFrameTime() const;
    static ReanimatorTransform LerpTransform(const ReanimatorTransform& a,
                                             const ReanimatorTransform& b,
                                             float t);

    const ReanimatorDefinition* def_ = nullptr;

    float animRate_ = 0.0f;
    float animTime_ = 0.0f;
    float lastFrameTime_ = -1.0f;
    ReanimLoopType loopType_ = ReanimLoopType::Loop;
    bool dead_ = false;

    int loopCount_ = 0;
    int frameStart_ = 0;
    int frameCount_ = 0;

    Transform overlay_{};
    std::vector<TrackInstance> tracks_;

    bool blending_ = false;
    float blendDuration_ = 0.0f;
    float blendElapsed_ = 0.0f;
    std::vector<ReanimatorTransform> blendFrom_;
    std::vector<ReanimatorTransform> blendTo_;
};
