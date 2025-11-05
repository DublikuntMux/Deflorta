#pragma once

#include <cstdint>
#include <vector>

struct PixelData
{
    std::vector<uint8_t> pixels;
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t pitch = 0;
};
