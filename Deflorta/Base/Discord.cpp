#include "Discord.hpp"

#include <ctime>
#include <iostream>

#include "Time.hpp"

namespace
{
    void HandleDiscordDisconnected(int, const char* msg)
    {
        std::cout << "Discord disconnected: " << msg << "\n";
    }

    void HandleDiscordErrored(int, const char* err)
    {
        std::cout << "Discord errored: " << err << "\n";
    }
}

std::string Discord::state_;
std::string Discord::details_;
int64_t Discord::startTimestamp_;
DiscordRichPresence Discord::presence_{};

void Discord::Initialize()
{
    DiscordEventHandlers handlers{};
    handlers.disconnected = HandleDiscordDisconnected;
    handlers.errored = HandleDiscordErrored;

    Discord_Initialize("1429291399379030048", &handlers, 1, nullptr);
    SetPresence("Loading");
}

void Discord::Shutdown()
{
    Discord_Shutdown();
}

void Discord::Update()
{
    Discord_RunCallbacks();
}

void Discord::SetPresence(const std::string& state, const std::string& details, bool resetTime)
{
    state_ = state;
    details_ = details;
    if (resetTime)
    {
        startTimestamp_ = time(nullptr);
    }
    ClearPresence();
    Discord_UpdatePresence(&presence_);
}

void Discord::ClearPresence()
{
    std::memset(&presence_, 0, sizeof(presence_));
    if (!state_.empty()) presence_.state = state_.c_str();
    if (!details_.empty()) presence_.details = details_.c_str();
    presence_.startTimestamp = startTimestamp_;
    presence_.largeImageKey = "cover";
    presence_.smallImageKey = "small";
}
