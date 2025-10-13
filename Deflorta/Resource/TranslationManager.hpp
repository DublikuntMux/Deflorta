#pragma once

#include <string>
#include <unordered_map>
#include <mutex>

class TranslationManager final
{
public:
    static bool Load(const std::string& filePath);
    static std::string Get(const std::string& key);
    static std::string Format(const std::string& key, const std::unordered_map<std::string, std::string>& bindings);

private:
    static std::unordered_map<std::string, std::string> translations_;
    static std::mutex mutex_;

    static void ReplaceAll(std::string& str, const std::string& from, const std::string& to);
};
