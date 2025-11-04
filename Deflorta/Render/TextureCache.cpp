#include "TextureCache.hpp"

IRenderBackend* TextureCache::backend_ = nullptr;
std::unordered_map<std::string, std::shared_ptr<ITexture>> TextureCache::textures_;
std::mutex TextureCache::mutex_;

void TextureCache::SetBackend(IRenderBackend* backend)
{
    std::lock_guard lock(mutex_);
    backend_ = backend;
}

IRenderBackend* TextureCache::GetBackend()
{
    return backend_;
}

std::shared_ptr<ITexture> TextureCache::GetTexture(const std::string& id)
{
    std::lock_guard lock(mutex_);
    if (const auto it = textures_.find(id); it != textures_.end())
        return it->second;
    return nullptr;
}

void TextureCache::AddTexture(const std::string& id, std::shared_ptr<ITexture> texture)
{
    std::lock_guard lock(mutex_);
    textures_[id] = std::move(texture);
}

void TextureCache::Clear()
{
    std::lock_guard lock(mutex_);
    textures_.clear();
}

std::shared_ptr<ITexture> TextureCache::CreateTexture(const PixelData& data)
{
    if (!backend_) return nullptr;
    return backend_->CreateTexture(data);
}
