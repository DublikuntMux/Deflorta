#pragma once

#include "IRenderBackend.hpp"

#include <string>
#include <unordered_map>
#include <memory>
#include <mutex>

class TextureCache
{
public:
    static void SetBackend(IRenderBackend* backend);
    static IRenderBackend* GetBackend();

    static std::shared_ptr<ITexture> GetTexture(const std::string& id);
    static void AddTexture(const std::string& id, std::shared_ptr<ITexture> texture);
    static void Clear();

    static std::shared_ptr<ITexture> CreateTexture(const PixelData& data);

private:
    static IRenderBackend* backend_;
    static std::unordered_map<std::string, std::shared_ptr<ITexture>> textures_;
    static std::mutex mutex_;
};
