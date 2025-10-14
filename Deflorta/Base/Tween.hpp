#pragma once

#include <functional>

enum class TweenMode: std::uint8_t
{
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut
};

struct TweenProperty
{
    float start;
    float end;
    std::function<void(float)> setter;
    TweenMode mode = TweenMode::Linear;
};

class Tween
{
public:
    Tween(std::vector<TweenProperty> properties, float duration);

    void Start();
    void Update();

    [[nodiscard]] bool IsActive() const;

private:
    std::vector<TweenProperty> properties_;
    float duration_;
    float elapsed_;
    bool active_;

    static float Interpolate(float t, TweenMode mode);
};
