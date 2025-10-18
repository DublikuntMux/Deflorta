#pragma once

#include <string>
#include <vector>
#include <optional>

#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Transform.hpp"

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
    bool truncateDisappearing = true;
    int renderGroup = 0;
    float shakeOverride = 0.0f;
    float shakeX = 0.0f;
    float shakeY = 0.0f;
};

class Reanimator final
{
public:
    explicit Reanimator(const ReanimatorDefinition* def);

    void PlayLayer(const std::string& trackName,
                   ReanimLoopType loopType = ReanimLoopType::Loop,
                   float animRate = 0.0f);

    void SetFramesForLayer(const std::string& trackName);
    [[nodiscard]] std::pair<int, int> GetFramesForLayer(const std::string& trackName) const;

    void Update();
    void Draw() const;

    void SetPosition(float x, float y);
    void OverrideScale(float sx, float sy);

    [[nodiscard]] bool IsDead() const { return dead_; }

private:
    [[nodiscard]] FrameTime GetFrameTime() const;
    static ReanimatorTransform LerpTransform(const ReanimatorTransform& a,
                                             const ReanimatorTransform& b,
                                             float t,
                                             bool truncateDisappear);

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
};
