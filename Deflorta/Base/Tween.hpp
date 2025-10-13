#pragma once

#include <functional>

enum class TweenMode
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

    void start();
    void update();

    [[nodiscard]] bool isActive() const;

private:
    std::vector<TweenProperty> properties_;
    float duration_;
    float elapsed_;
    bool active_;
};
