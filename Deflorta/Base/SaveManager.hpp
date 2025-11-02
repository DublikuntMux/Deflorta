#pragma once

#include <string>
#include <unordered_map>
#include <variant>

class SaveManager final
{
public:
    using Value = std::variant<int, float, bool, std::string>;

    static void Initialize();
    static void Uninitialize();

    static bool Load();
    static bool Save();

    static void SetInt(const std::string& key, int value);
    static void SetFloat(const std::string& key, float value);
    static void SetBool(const std::string& key, bool value);
    static void SetString(const std::string& key, const std::string& value);

    static int GetInt(const std::string& key, int default_value = 0);
    static float GetFloat(const std::string& key, float default_value = 0.0f);
    static bool GetBool(const std::string& key, bool default_value = false);
    static std::string GetString(const std::string& key, const std::string& default_value = "");

    static bool HasKey(const std::string& key);
    static void RemoveKey(const std::string& key);
    static void Clear();

private:
    static std::string save_file_path_;
    static std::unordered_map<std::string, Value> data_;
    static bool initialized_;
};
