#include "Utils.hpp"

#include <filesystem>

#ifdef _WIN32
#include <Windows.h>
#endif

namespace Utils
{
    uint32_t NextPowerOf2(uint32_t n)
    {
        if (n == 0) return 1;

        if ((n & n - 1) == 0)
            return n;

        n--;
        n |= n >> 1;
        n |= n >> 2;
        n |= n >> 4;
        n |= n >> 8;
        n |= n >> 16;
        n++;

        return n;
    }

    std::string GetExecutableDir()
    {
#ifdef _WIN32
        char exePath[MAX_PATH];
        const DWORD len = GetModuleFileNameA(nullptr, exePath, MAX_PATH);
        if (len == 0)
            return {"./"};
        try
        {
            return std::filesystem::path(std::string(exePath, len)).parent_path().string();
        }
        catch (...)
        {
            return {"./"};
        }
#else
        try
        {
            return std::filesystem::current_path().string();
        }
        catch (...)
        {
            return {"./"};
        }
#endif
    }
}
