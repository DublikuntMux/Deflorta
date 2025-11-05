#include "Tween.hpp"

#include "Time.hpp"

Tween::Tween(std::vector<TweenProperty> properties, float duration)
    : properties_(std::move(properties)), duration_(duration), elapsed_(0.0f), active_(false)
{
}

void Tween::Start()
{
    elapsed_ = 0.0f;
    active_ = true;

    for (auto& prop : properties_)
    {
        if (prop.setter)
            prop.setter(prop.start);
    }
}

void Tween::Update()
{
    if (!active_) return;

    elapsed_ += Time::GetDeltaTime();
    const float t = elapsed_ / duration_;

    if (t >= 1.0f)
    {
        for (auto& prop : properties_)
        {
            if (prop.setter)
                prop.setter(prop.end);
        }
        active_ = false;
    }
    else
    {
        for (auto& [start, end, setter, mode] : properties_)
        {
            const float inter = Interpolate(t, mode);
            const float value = start + (end - start) * inter;
            if (setter)
                setter(value);
        }
    }
}

bool Tween::IsActive() const
{
    return active_;
}

float Tween::Interpolate(float t, TweenMode mode)
{
    switch (mode)
    {
    case TweenMode::Linear:
        return t;
    case TweenMode::EaseIn:
        return t * t;
    case TweenMode::EaseOut:
        return t * (2 - t);
    case TweenMode::EaseInOut:
        return t < 0.5f ? 2 * t * t : -1 + (4 - 2 * t) * t;
    }
    return t;
}
