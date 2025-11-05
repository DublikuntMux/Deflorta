#include "Reanimator.hpp"

#include "../Resource/ResourceManager.hpp"
#include "Renderer.hpp"
#include "../Base/Time.hpp"

#include <cmath>
#include <algorithm>
#include <utility>

Reanimator::Reanimator(const ReanimatorDefinition* def)
{
    def_ = def;
    animRate_ = def_->fps;
    frameStart_ = 0;
    frameCount_ = !def->tracks.empty() ? static_cast<int>(def->tracks.front().transforms.size()) : 0;
    tracks_.resize(def->tracks.size());
}

bool Reanimator::IsFinished() const
{
    if (frameCount_ <= 0) return true;

    switch (loopType_)
    {
    case ReanimLoopType::Loop:
    case ReanimLoopType::LoopFullLastFrame:
        return false;
    case ReanimLoopType::PlayOnce:
    case ReanimLoopType::PlayOnceFullLastFrame:
        return dead_;
    case ReanimLoopType::PlayOnceAndHold:
    case ReanimLoopType::PlayOnceFullLastFrameAndHold:
        {
            if (animRate_ >= 0.0f)
                return animTime_ >= 1.0f;
            return animTime_ <= 0.0f;
        }
    }
    return false;
}

void Reanimator::PlayLayer(const std::string& trackName, ReanimLoopType loopType, float animRate, float blendTime)
{
    blendFrom_.clear();
    blendTo_.clear();
    blending_ = false;
    blendElapsed_ = 0.0f;
    blendDuration_ = std::max(0.0f, blendTime);

    if (def_ && !def_->tracks.empty())
    {
        const auto [before, after, frac] = GetFrameTime();
        blendFrom_.reserve(def_->tracks.size());
        for (size_t ti = 0; ti < def_->tracks.size(); ++ti)
        {
            const auto& transforms = def_->tracks[ti].transforms;
            const auto& a = transforms[before];
            const auto& b = transforms[after];
            auto cur = LerpTransform(a, b, frac);
            if (ti < tracks_.size() && tracks_[ti].imageOverride.has_value())
            {
                cur.image = *tracks_[ti].imageOverride;
            }
            blendFrom_.push_back(cur);
        }
    }

    loopType_ = loopType;
    if (animRate != 0.0f) animRate_ = animRate;
    loopCount_ = 0;
    dead_ = false;

    SetFramesForLayer(trackName);

    if (def_ && !def_->tracks.empty() && blendDuration_ > 0.0f)
    {
        const int firstIndex = frameStart_;
        blendTo_.reserve(def_->tracks.size());
        for (size_t ti = 0; ti < def_->tracks.size(); ++ti)
        {
            const auto& transforms = def_->tracks[ti].transforms;
            ReanimatorTransform tgt = transforms[std::clamp(firstIndex, 0, static_cast<int>(transforms.size()) - 1)];
            if (ti < tracks_.size() && tracks_[ti].imageOverride.has_value())
            {
                tgt.image = *tracks_[ti].imageOverride;
            }
            blendTo_.push_back(tgt);
        }
        if (!blendFrom_.empty())
        {
            blending_ = true;
            blendElapsed_ = 0.0f;
        }
    }

    animTime_ = animRate_ >= 0.0f ? 0.0f : 0.999f;
    lastFrameTime_ = -1.0f;
}

void Reanimator::SetFramesForLayer(const std::string& trackName)
{
    auto [s, c] = GetFramesForLayer(trackName);
    frameStart_ = s;
    frameCount_ = c;
}

int Reanimator::FindTrackIndexByName(const std::string& trackName) const
{
    if (!def_) return -1;
    for (size_t i = 0; i < def_->tracks.size(); ++i)
    {
        if (def_->tracks[i].name == trackName) return static_cast<int>(i);
    }
    return -1;
}

void Reanimator::OverrideLayerImage(const std::string& trackName, const std::string& image)
{
    const int idx = FindTrackIndexByName(trackName);
    if (idx < 0 || std::cmp_greater_equal(idx, tracks_.size())) return;
    tracks_[static_cast<size_t>(idx)].imageOverride = image;
}

void Reanimator::ClearLayerImageOverride(const std::string& trackName)
{
    const int idx = FindTrackIndexByName(trackName);
    if (idx < 0 || std::cmp_greater_equal(idx, tracks_.size())) return;
    tracks_[static_cast<size_t>(idx)].imageOverride.reset();
}

void Reanimator::SetLayerVisible(const std::string& trackName, bool visible)
{
    const int idx = FindTrackIndexByName(trackName);
    if (idx < 0 || std::cmp_greater_equal(idx, tracks_.size())) return;
    tracks_[static_cast<size_t>(idx)].visible = visible;
}

void Reanimator::SetLayerZ(const std::string& trackName, int z)
{
    const int idx = FindTrackIndexByName(trackName);
    if (idx < 0 || std::cmp_greater_equal(idx, tracks_.size())) return;
    tracks_[static_cast<size_t>(idx)].renderGroup = z;
}

void Reanimator::SetAllLayersZ(int z)
{
    for (auto& track : tracks_)
    {
        track.renderGroup = z;
    }
}

std::pair<int, int> Reanimator::GetFramesForLayer(const std::string& trackName) const
{
    int start = 0;
    int end = 0;
    bool started = false;

    const ReanimatorTrack* target = nullptr;
    for (const auto& tr : def_->tracks)
    {
        if (tr.name == trackName)
        {
            target = &tr;
            break;
        }
    }
    if (!target)
    {
        int count = !def_->tracks.empty() ? static_cast<int>(def_->tracks.front().transforms.size()) : 0;
        return {0, count};
    }

    const int n = static_cast<int>(target->transforms.size());
    for (int i = 0; i < n; ++i)
    {
        if (target->transforms[i].frame >= 0.0f)
        {
            if (!started)
            {
                start = i;
                started = true;
            }
            end = i;
        }
    }
    if (!started) return {0, 0};
    return {start, end - start + 1};
}

void Reanimator::Update()
{
    if (dead_ || frameCount_ == 0)
    {
        for (auto& t : tracks_)
        {
            if (t.shakeOverride != 0.0f)
            {
                t.shake = {t.shakeOverride * 0.5f, -t.shakeOverride * 0.5f};
            }
            else { t.shake = {0.0f, 0.0f}; }
        }
        return;
    }

    if (blending_)
    {
        blendElapsed_ += Time::GetDeltaTime();
        if (blendElapsed_ >= blendDuration_)
        {
            blending_ = false;
        }
    }

    if (!blending_)
    {
        lastFrameTime_ = animTime_;
        animTime_ += Time::GetDeltaTime() * animRate_ / static_cast<float>(std::max(1, frameCount_));

        if (animRate_ >= 0.0f)
        {
            if (animTime_ >= 1.0f)
            {
                switch (loopType_)
                {
                case ReanimLoopType::Loop:
                case ReanimLoopType::LoopFullLastFrame:
                    while (animTime_ >= 1.0f)
                    {
                        animTime_ -= 1.0f;
                        ++loopCount_;
                    }
                    break;
                case ReanimLoopType::PlayOnce:
                case ReanimLoopType::PlayOnceFullLastFrame:
                    animTime_ = 1.0f;
                    dead_ = true;
                    loopCount_ = 1;
                    break;
                case ReanimLoopType::PlayOnceAndHold:
                case ReanimLoopType::PlayOnceFullLastFrameAndHold:
                    animTime_ = 1.0f;
                    break;
                }
            }
        }
        else
        {
            if (animTime_ < 0.0f)
            {
                switch (loopType_)
                {
                case ReanimLoopType::Loop:
                case ReanimLoopType::LoopFullLastFrame:
                    while (animTime_ < 0.0f)
                    {
                        animTime_ += 1.0f;
                        ++loopCount_;
                    }
                    break;
                case ReanimLoopType::PlayOnce:
                case ReanimLoopType::PlayOnceFullLastFrame:
                    animTime_ = 0.0f;
                    dead_ = true;
                    loopCount_ = 1;
                    break;
                case ReanimLoopType::PlayOnceAndHold:
                case ReanimLoopType::PlayOnceFullLastFrameAndHold:
                    animTime_ = 0.0f;
                    break;
                }
            }
        }
    }

    for (auto& t : tracks_)
    {
        if (t.shakeOverride != 0.0f)
        {
            t.shake = {t.shakeOverride * 0.5f, -t.shakeOverride * 0.5f};
        }
        else { t.shake = {0.0f, 0.0f}; }
    }
}

void Reanimator::Draw() const
{
    if (!def_ || frameCount_ == 0) return;

    if (blending_ && blendDuration_ > 0.0f && blendElapsed_ < blendDuration_ && blendFrom_.size() == def_->tracks.size()
        && blendTo_.size() == def_->tracks.size())
    {
        const float f = std::clamp(blendElapsed_ / std::max(0.0001f, blendDuration_), 0.0f, 1.0f);
        for (size_t ti = 0; ti < def_->tracks.size(); ++ti)
        {
            const auto& name = def_->tracks[ti].name;
            if (ti < tracks_.size() && !tracks_[ti].visible) continue;

            auto cur = LerpTransform(blendFrom_[ti], blendTo_[ti], f);

            if (ti < tracks_.size() && tracks_[ti].imageOverride.has_value())
            {
                cur.image = *tracks_[ti].imageOverride;
            }

            if (cur.alpha <= 0.0f) continue;

            const int z = ti < tracks_.size() ? tracks_[ti].renderGroup : 0;

            if (!cur.image.empty() && cur.frame >= 0.0f)
            {
                if (auto bmp = ResourceManager::GetImage(cur.image))
                {
                    cur.translation += overlay_.position + tracks_[ti].shake;
                    cur.scale *= overlay_.scale;

                    Renderer::EnqueueReanim(bmp, cur, z);
                }
            }
            else if (!cur.text.empty() && !cur.font.empty())
            {
                const std::wstring text(cur.text.begin(), cur.text.end());
                const std::wstring font = ResourceManager::GetFont(cur.font);
                const glm::vec2 textPos = overlay_.position + cur.translation + tracks_[ti].shake;
                const float size = 16.0f * overlay_.scale.y * cur.scale.y;
                const auto rect = Rect(textPos.x - 200.0f, textPos.y - size, textPos.x + 200.0f, textPos.y + size);
                const auto color = Color(1.f, 1.f, 1.f, std::clamp(cur.alpha, 0.0f, 1.0f));

                Renderer::EnqueueTextW(text, rect, font.empty() ? L"Consolas" : font, size, color, z);
            }
            else if (name == "fullscreen")
            {
                //TODO: background fill
            }
        }
    }
    else
    {
        const auto [before, after, frac] = GetFrameTime();

        for (size_t ti = 0; ti < def_->tracks.size(); ++ti)
        {
            const auto& [name, transforms] = def_->tracks[ti];
            if (transforms.empty()) continue;

            if (ti < tracks_.size() && !tracks_[ti].visible) continue;

            const auto& a = transforms[before];
            const auto& b = transforms[after];
            auto cur = LerpTransform(a, b, frac);

            if (ti < tracks_.size() && tracks_[ti].imageOverride.has_value())
            {
                cur.image = *tracks_[ti].imageOverride;
            }

            if (cur.alpha <= 0.0f) continue;

            const int z = ti < tracks_.size() ? tracks_[ti].renderGroup : 0;

            if (!cur.image.empty() && cur.frame >= 0.0f)
            {
                if (auto bmp = ResourceManager::GetImage(cur.image))
                {
                    cur.translation += overlay_.position + tracks_[ti].shake;
                    cur.scale *= overlay_.scale;

                    Renderer::EnqueueReanim(bmp, cur, z);
                }
            }
            else if (!cur.text.empty() && !cur.font.empty())
            {
                const std::wstring text(cur.text.begin(), cur.text.end());
                const std::wstring font = ResourceManager::GetFont(cur.font);
                const glm::vec2 textPos = overlay_.position + cur.translation + tracks_[ti].shake;
                const float size = 16.0f * overlay_.scale.y * cur.scale.y;
                const auto rect = Rect(textPos.x - 200.0f, textPos.y - size, textPos.x + 200.0f, textPos.y + size);
                const auto color = Color(1.f, 1.f, 1.f, std::clamp(cur.alpha, 0.0f, 1.0f));

                Renderer::EnqueueTextW(text, rect, font.empty() ? L"Consolas" : font, size, color, z);
            }
            else if (name == "fullscreen")
            {
                //TODO: background fill
            }
        }
    }
}

void Reanimator::SetPosition(glm::vec2 pos)
{
    overlay_.position = pos;
}

void Reanimator::OverrideScale(glm::vec2 scale)
{
    overlay_.scale = scale;
}

FrameTime Reanimator::GetFrameTime() const
{
    FrameTime ft;
    if (frameCount_ <= 0) return ft;

    const bool fullLast = loopType_ == ReanimLoopType::LoopFullLastFrame ||
        loopType_ == ReanimLoopType::PlayOnceFullLastFrame ||
        loopType_ == ReanimLoopType::PlayOnceFullLastFrameAndHold;

    const int start = frameStart_;
    const int count = frameCount_;
    const int span = fullLast ? count - 1 : std::max(1, count - 1);

    float t = animTime_;
    if (!fullLast)
    {
        t = std::min(0.9999f, std::max(0.0f, t));
    }
    const float fIndex = t * static_cast<float>(span);
    const int before = start + static_cast<int>(std::floor(fIndex));
    const int after = std::min(start + count - 1, before + 1);
    const float frac = fIndex - std::floor(fIndex);

    ft.before = before;
    ft.after = after;
    ft.frac = frac;
    return ft;
}

ReanimatorTransform Reanimator::LerpTransform(const ReanimatorTransform& a,
                                              const ReanimatorTransform& b,
                                              float t)
{
    auto lerp = [&](float av, float bv)
    {
        return av + (bv - av) * t;
    };

    ReanimatorTransform r = a;

    r.translation.x = lerp(a.translation.x, b.translation.x);
    r.translation.y = lerp(a.translation.y, b.translation.y);
    r.skew.x = lerp(a.skew.x, b.skew.x);
    r.skew.y = lerp(a.skew.y, b.skew.y);
    r.scale.x = lerp(a.scale.x, b.scale.x);
    r.scale.y = lerp(a.scale.y, b.scale.y);
    r.frame = lerp(a.frame, b.frame);
    r.alpha = lerp(a.alpha, b.alpha);

    if (a.frame >= 0.0f && b.frame < 0.0f && t > 0.0f)
        r.frame = -1.0f;

    return r;
}
