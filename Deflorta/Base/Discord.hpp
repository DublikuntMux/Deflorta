#pragma once

#include <string>

#include <discord_rpc.h>

class Discord final
{
public:
    static void Initialize();
    static void Shutdown();
    static void Update();
    static void SetPresence(const std::string& state, const std::string& details = "", bool resetTime = true);

private:
    static void ClearPresence();

    static std::string state_;
    static std::string details_;
    static int64_t startTimestamp_;
    static DiscordRichPresence presence_;
};
