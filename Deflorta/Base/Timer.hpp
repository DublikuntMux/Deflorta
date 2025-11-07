#pragma once

#include <functional>

class Scene;

class Timer
{
public:
    explicit Timer(Scene* scene = nullptr);
    ~Timer() = default;

    void Update();
    void Start(float timeSec = -1.0f);
    void Stop();
    [[nodiscard]] bool IsStopped() const;

    void SetWaitTime(float waitTime);
    [[nodiscard]] float GetWaitTime() const { return waitTime_; }

    void SetAutostart(bool autostart) { autostart_ = autostart; }
    [[nodiscard]] bool HasAutostart() const { return autostart_; }

    void SetOneShot(bool oneShot) { oneShot_ = oneShot; }
    [[nodiscard]] bool IsOneShot() const { return oneShot_; }

    void SetPaused(bool paused) { paused_ = paused; }
    [[nodiscard]] bool IsPaused() const { return paused_; }

    void SetIgnoreTimeScale(bool ignore) { ignoreTimeScale_ = ignore; }
    [[nodiscard]] bool IsIgnoringTimeScale() const { return ignoreTimeScale_; }

    [[nodiscard]] float GetTimeLeft() const { return timeLeft_; }

    void SetTimeoutCallback(std::function<void()> callback) { timeoutCallback_ = std::move(callback); }
    void EmitTimeout() const;

    void SetScene(Scene* scene) { scene_ = scene; }
    [[nodiscard]] Scene* GetScene() const { return scene_; }

private:
    Scene* scene_ = nullptr;
    std::function<void()> timeoutCallback_;
    
    float waitTime_ = 1.0f;
    float timeLeft_ = 0.0f;
    
    bool autostart_ = false;
    bool oneShot_ = false;
    bool paused_ = false;
    bool ignoreTimeScale_ = false;
    bool running_ = false;
    bool hasStarted_ = false;
};
