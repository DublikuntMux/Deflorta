#include "SaveManager.hpp"

#include <pugixml.hpp>
#include <Windows.h>
#include <ShlObj.h>

#include <filesystem>

std::string SaveManager::save_file_path_;
std::unordered_map<std::string, SaveManager::Value> SaveManager::data_;
bool SaveManager::initialized_ = false;

void SaveManager::Initialize()
{
    if (initialized_)
        return;

    PWSTR documents_path = nullptr;
    const HRESULT hr = SHGetKnownFolderPath(FOLDERID_Documents, 0, nullptr, &documents_path);

    if (SUCCEEDED(hr) && documents_path)
    {
        const int size_needed = WideCharToMultiByte(CP_UTF8, 0, documents_path, -1, nullptr, 0, nullptr, nullptr);
        std::string documents_str(size_needed - 1, 0);
        WideCharToMultiByte(CP_UTF8, 0, documents_path, -1, &documents_str[0], size_needed, nullptr, nullptr);

        CoTaskMemFree(documents_path);

        const std::filesystem::path save_dir = std::filesystem::path(documents_str) / "My Games" / "Deflorta";

        try
        {
            std::filesystem::create_directories(save_dir);
        }
        catch (const std::filesystem::filesystem_error&)
        {
            save_file_path_ = "save.xml";
            initialized_ = true;
            Load();
            return;
        }

        save_file_path_ = (save_dir / "save.xml").string();
    }
    else
    {
        save_file_path_ = "save.xml";
    }

    initialized_ = true;
    Load();
}

void SaveManager::Uninitialize()
{
    if (!initialized_)
        return;

    Save();

    data_.clear();
    initialized_ = false;
}

bool SaveManager::Load()
{
    if (!initialized_)
        return false;

    pugi::xml_document doc;

    if (const pugi::xml_parse_result result = doc.load_file(save_file_path_.c_str()); !result)
    {
        return false;
    }

    data_.clear();

    const pugi::xml_node root = doc.child("SaveData");
    if (!root)
        return false;

    for (pugi::xml_node entry : root.children("Entry"))
    {
        std::string key = entry.attribute("key").as_string();
        std::string type = entry.attribute("type").as_string();
        std::string value_str = entry.attribute("value").as_string();

        if (key.empty())
            continue;

        if (type == "int")
        {
            data_[key] = std::stoi(value_str);
        }
        else if (type == "float")
        {
            data_[key] = std::stof(value_str);
        }
        else if (type == "bool")
        {
            data_[key] = value_str == "true" || value_str == "1";
        }
        else if (type == "string")
        {
            data_[key] = value_str;
        }
    }

    return true;
}

bool SaveManager::Save()
{
    if (!initialized_)
        return false;

    pugi::xml_document doc;

    pugi::xml_node declaration = doc.prepend_child(pugi::node_declaration);
    declaration.append_attribute("version") = "1.0";
    declaration.append_attribute("encoding") = "UTF-8";

    pugi::xml_node root = doc.append_child("SaveData");

    for (const auto& [key, value] : data_)
    {
        pugi::xml_node entry = root.append_child("Entry");
        entry.append_attribute("key") = key.c_str();

        if (std::holds_alternative<int>(value))
        {
            entry.append_attribute("type") = "int";
            entry.append_attribute("value") = std::get<int>(value);
        }
        else if (std::holds_alternative<float>(value))
        {
            entry.append_attribute("type") = "float";
            entry.append_attribute("value") = std::get<float>(value);
        }
        else if (std::holds_alternative<bool>(value))
        {
            entry.append_attribute("type") = "bool";
            entry.append_attribute("value") = std::get<bool>(value) ? "true" : "false";
        }
        else if (std::holds_alternative<std::string>(value))
        {
            entry.append_attribute("type") = "string";
            entry.append_attribute("value") = std::get<std::string>(value).c_str();
        }
    }

    const bool result = doc.save_file(save_file_path_.c_str(), "  ");
    return result;
}

void SaveManager::SetInt(const std::string& key, int value)
{
    if (!initialized_)
        return;
    data_[key] = value;
}

void SaveManager::SetFloat(const std::string& key, float value)
{
    if (!initialized_)
        return;
    data_[key] = value;
}

void SaveManager::SetBool(const std::string& key, bool value)
{
    if (!initialized_)
        return;
    data_[key] = value;
}

void SaveManager::SetString(const std::string& key, const std::string& value)
{
    if (!initialized_)
        return;
    data_[key] = value;
}

int SaveManager::GetInt(const std::string& key, int default_value)
{
    if (!initialized_)
        return default_value;

    const auto it = data_.find(key);
    if (it == data_.end())
        return default_value;

    if (std::holds_alternative<int>(it->second))
        return std::get<int>(it->second);

    return default_value;
}

float SaveManager::GetFloat(const std::string& key, float default_value)
{
    if (!initialized_)
        return default_value;

    const auto it = data_.find(key);
    if (it == data_.end())
        return default_value;

    if (std::holds_alternative<float>(it->second))
        return std::get<float>(it->second);

    return default_value;
}

bool SaveManager::GetBool(const std::string& key, bool default_value)
{
    if (!initialized_)
        return default_value;

    const auto it = data_.find(key);
    if (it == data_.end())
        return default_value;

    if (std::holds_alternative<bool>(it->second))
        return std::get<bool>(it->second);

    return default_value;
}

std::string SaveManager::GetString(const std::string& key, const std::string& default_value)
{
    if (!initialized_)
        return default_value;

    const auto it = data_.find(key);
    if (it == data_.end())
        return default_value;

    if (std::holds_alternative<std::string>(it->second))
        return std::get<std::string>(it->second);

    return default_value;
}

bool SaveManager::HasKey(const std::string& key)
{
    if (!initialized_)
        return false;

    return data_.contains(key);
}

void SaveManager::RemoveKey(const std::string& key)
{
    if (!initialized_)
        return;

    data_.erase(key);
}

void SaveManager::Clear()
{
    if (!initialized_)
        return;

    data_.clear();
}
