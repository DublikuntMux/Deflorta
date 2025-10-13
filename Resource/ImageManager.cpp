#pragma once
#include <string>
#include <unordered_map>
#include <vector>

struct ImageData {
    int width;
    int height;
    std::vector<unsigned char> pixels; // RGBA
};

class ImageManager {
public:
    ImageManager();
    ~ImageManager();

    void PreloadImage(const std::string& filename);
    const ImageData* GetImage(const std::string& filename);
    void UnloadImage(const std::string& filename);
    void UnloadAll();

private:
    ImageData* LoadImage(const std::string& filename);
    std::unordered_map<std::string, ImageData*> m_images;
};

