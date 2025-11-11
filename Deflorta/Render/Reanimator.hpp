#pragma once

#include "Layer.hpp"
#include "../Resource/ReanimationLoader.hpp"
#include "../Base/Transform.hpp"

#include <string>
#include <vector>
#include <optional>

#include "../Base/Color.hpp"

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
    glm::vec2 shake = {0.0f, 0.0f};
    std::optional<std::string> imageOverride;
    bool visible = true;
    float opacity = 1.0f;
    Color tint = Color::White;
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

    void SetPosition(glm::vec2 pos);
    void OverrideScale(glm::vec2 scale);
    void OverrideLayerImage(std::string trackName, std::string image);
    void ClearLayerImageOverride(const std::string& trackName);

    void SetLayerVisible(const std::string& trackName, bool visible);

    void SetLayerZ(const std::string& trackName, int z);
    void SetAllLayersZ(int z);

    void SetLayerOpacity(const std::string& trackName, float opacity);
    void SetAllLayersOpacity(float opacity);

    void SetLayerTint(const std::string& trackName, const Color& tint);
    void SetAllLayersTint(const Color& tint);
    void ResetLayerTint(const std::string& trackName);
    void ResetAllLayersTint();

    void SetTint(const Color& tint);
    void ResetTint();

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

    Color globalTint_ = Color::White;

    bool blending_ = false;
    float blendDuration_ = 0.0f;
    float blendElapsed_ = 0.0f;
    std::vector<ReanimatorTransform> blendFrom_;
    std::vector<ReanimatorTransform> blendTo_;
};
