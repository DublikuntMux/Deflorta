#pragma once

#include <functional>

class Timer
{
public:
    void Update();
    void Start(float timeSec = -1.0f);
    void Stop();
    [[nodiscard]] bool IsStopped() const;

    void SetWaitTime(float waitTime);
    [[nodiscard]] float GetWaitTime() const { return waitTime_; }

    void SetOneShot(bool oneShot) { oneShot_ = oneShot; }
    [[nodiscard]] bool IsOneShot() const { return oneShot_; }

    void SetPaused(bool paused) { paused_ = paused; }
    [[nodiscard]] bool IsPaused() const { return paused_; }

    [[nodiscard]] float GetTimeLeft() const { return timeLeft_; }

    void SetTimeoutCallback(std::function<void()> callback) { timeoutCallback_ = std::move(callback); }

private:
    std::function<void()> timeoutCallback_;

    float waitTime_ = 1.0f;
    float timeLeft_ = 0.0f;

    bool oneShot_ = false;
    bool paused_ = false;
    bool running_ = false;
    bool hasStarted_ = false;
};
