#include "Reanimator.hpp"

#include <cmath>
#include <algorithm>
#include <iostream>

#include "../Resource/ResourceManager.hpp"
#include "Renderer.hpp"
#include "../Base/Time.hpp"

Reanimator::Reanimator(const ReanimatorDefinition* def)
{
    def_ = def;
    animRate_ = def_->fps;
    frameStart_ = 0;
    frameCount_ = !def->tracks.empty() ? static_cast<int>(def->tracks.front().transforms.size()) : 0;
    tracks_.resize(def->tracks.size());
}

void Reanimator::PlayLayer(const std::string& trackName, ReanimLoopType loopType, float animRate)
{
    loopType_ = loopType;
    if (animRate != 0.0f) animRate_ = animRate;
    loopCount_ = 0;
    dead_ = false;
    SetFramesForLayer(trackName);
    animTime_ = animRate_ >= 0.0f ? 0.0f : 0.999f;
    lastFrameTime_ = -1.0f;
}

void Reanimator::SetFramesForLayer(const std::string& trackName)
{
    auto [s, c] = GetFramesForLayer(trackName);
    frameStart_ = s;
    frameCount_ = c;
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
    if (dead_ || frameCount_ == 0) return;

    lastFrameTime_ = animTime_;
    animTime_ += Time::GetDeltaTime() * animRate_ / std::max(1, frameCount_);

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

    for (auto& t : tracks_)
    {
        if (t.shakeOverride != 0.0f)
        {
            t.shakeX = t.shakeOverride * 0.5f;
            t.shakeY = -t.shakeOverride * 0.5f;
        }
        else { t.shakeX = t.shakeY = 0.0f; }
    }
}

void Reanimator::Draw() const
{
    if (!def_ || frameCount_ == 0) return;

    const auto [before, after, frac] = GetFrameTime();

    for (size_t ti = 0; ti < def_->tracks.size(); ++ti)
    {
        const auto& [name, transforms] = def_->tracks[ti];
        if (transforms.empty()) continue;

        const auto& a = transforms[before];
        const auto& b = transforms[after];
        const auto cur = LerpTransform(a, b, frac, tracks_[ti].truncateDisappearing);

        if (cur.alpha <= 0.0f) continue;

        if (!cur.image.empty() && cur.frame >= 0.0f)
        {
            if (auto* bmp = ResourceManager::GetImage(cur.image))
            {
                ReanimatorTransform t{};
                t.transX = overlay_.x + cur.transX + tracks_[ti].shakeX;
                t.transY = overlay_.y + cur.transY + tracks_[ti].shakeY;
                t.skewX = cur.skewX;
                t.skewY = cur.skewY;
                t.scaleX = overlay_.scaleX * cur.scaleX;
                t.scaleY = overlay_.scaleY * cur.scaleY;
                Renderer::DrawReanim(bmp, t, std::clamp(cur.alpha, 0.0f, 1.0f));
            }
        }
        else if (!cur.text.empty() && !cur.font.empty())
        {
            const std::wstring text(cur.text.begin(), cur.text.end());
            const std::wstring font = ResourceManager::GetFont(cur.font);
            const float x = overlay_.x + cur.transX + tracks_[ti].shakeX;
            const float y = overlay_.y + cur.transY + tracks_[ti].shakeY;
            const float size = 16.0f * overlay_.scaleY * cur.scaleY;
            const auto rect = D2D1::RectF(x - 200.0f, y - size, x + 200.0f, y + size);
            Renderer::DrawTextW(text, rect, font.empty() ? L"Consolas" : font, size,
                                D2D1::ColorF(1.f, 1.f, 1.f, std::clamp(cur.alpha, 0.0f, 1.0f)));
        }
        else if (name == "fullscreen")
        {
            // Not implemented: background fill
        }
    }
}

void Reanimator::SetPosition(float x, float y)
{
    overlay_.x = x;
    overlay_.y = y;
}

void Reanimator::OverrideScale(float sx, float sy)
{
    overlay_.scaleX = sx;
    overlay_.scaleY = sy;
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
    const float fIndex = t * span;
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
                                              float t,
                                              bool truncateDisappear)
{
    auto lerp = [&](float av, float bv)
    {
        return av + (bv - av) * t;
    };

    ReanimatorTransform r = a;

    r.transX = lerp(a.transX, b.transX);
    r.transY = lerp(a.transY, b.transY);
    r.skewX = lerp(a.skewX, b.skewX);
    r.skewY = lerp(a.skewY, b.skewY);
    r.scaleX = lerp(a.scaleX, b.scaleX);
    r.scaleY = lerp(a.scaleY, b.scaleY);
    r.frame = lerp(a.frame, b.frame);
    r.alpha = lerp(a.alpha, b.alpha);

    if (truncateDisappear && a.frame >= 0.0f && b.frame < 0.0f && t > 0.0f)
        r.frame = -1.0f;

    return r;
}
