#include "Timer.hpp"

#include "Time.hpp"

void Timer::Update()
{
    if (!hasStarted_)
    {
        hasStarted_ = true;
        Start();
    }

    if (!running_ || paused_)
        return;

    float deltaTime = Time::GetDeltaTime();
    timeLeft_ -= deltaTime;

    if (timeLeft_ <= 0.0f)
    {
        timeLeft_ = 0.0f;
        if (timeoutCallback_)
        {
            timeoutCallback_();
        }
        if (oneShot_)
        {
            running_ = false;
        }
        else
        {
            timeLeft_ = waitTime_;
        }
    }
}

void Timer::Start(float timeSec)
{
    if (timeSec > 0.0f)
    {
        waitTime_ = timeSec;
    }
    timeLeft_ = waitTime_;
    running_ = true;
    hasStarted_ = true;
}

void Timer::Stop()
{
    running_ = false;
    timeLeft_ = 0.0f;
}

bool Timer::IsStopped() const
{
    return !running_;
}

void Timer::SetWaitTime(float waitTime)
{
    waitTime_ = waitTime > 0.0f ? waitTime : 0.0f;
}
