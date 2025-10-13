#include "TranslationManager.hpp"

#include <fstream>
#include <sstream>
#include <filesystem>

std::unordered_map<std::string, std::string> TranslationManager::translations_;
std::mutex TranslationManager::mutex_;

bool TranslationManager::Load(const std::string& filePath)
{
    std::ifstream file(filePath, std::ios::binary);
    if (!file.is_open())
        return false;

    std::unordered_map<std::string, std::string> temp;

    std::string line;
    std::string currentKey;
    std::ostringstream currentValue;

    auto pushCurrent = [&]
    {
        if (!currentKey.empty())
        {
            std::string value = currentValue.str();
            if (!value.empty() && value.back() == '\n') value.pop_back();
            temp[currentKey] = std::move(value);
            currentKey.clear();
            currentValue.str("");
            currentValue.clear();
        }
    };

    while (std::getline(file, line))
    {
        if (!line.empty() && line.back() == '\r') line.pop_back();

        if (!line.empty() && line.front() == '[')
        {
            if (const auto pos = line.find(']'); pos != std::string::npos)
            {
                pushCurrent();
                currentKey = line.substr(1, pos - 1);
                continue;
            }
        }

        if (!currentKey.empty())
        {
            currentValue << line << '\n';
        }
    }

    pushCurrent();

    {
        std::lock_guard lock(mutex_);
        translations_.swap(temp);
    }

    return true;
}

std::string TranslationManager::Get(const std::string& key)
{
    std::lock_guard lock(mutex_);
    if (const auto it = translations_.find(key); it != translations_.end()) return it->second;
    return {};
}

std::string TranslationManager::Format(const std::string& key,
                                       const std::unordered_map<std::string, std::string>& bindings)
{
    std::string base = Get(key);
    if (base.empty() || bindings.empty()) return base;

    for (const auto& [name, value] : bindings)
    {
        const std::string token = std::string("{") + name + "}";
        ReplaceAll(base, token, value);
    }
    return base;
}

void TranslationManager::ReplaceAll(std::string& str, const std::string& from, const std::string& to)
{
    if (from.empty()) return;
    size_t start_pos = 0;
    while ((start_pos = str.find(from, start_pos)) != std::string::npos)
    {
        str.replace(start_pos, from.length(), to);
        start_pos += to.length();
    }
}
