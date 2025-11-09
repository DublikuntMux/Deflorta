#pragma once

#include "IRenderBackend.hpp"
#include "AtlasBuilder.hpp"
#include "../Base/Transform.hpp"

#include <memory>
#include <vector>
#include <string>

struct ReanimatorTransform;

enum class DrawType : std::uint8_t
{
    Image,
    ImageAtlas,
    Text,
    Rectangle
};

struct DrawItem
{
    struct ImageData
    {
        std::shared_ptr<ITexture> texture;
        glm::mat3 transform{};
        ImageData() = default;

        ImageData(std::shared_ptr<ITexture> tex, const glm::mat3& m)
            : texture(std::move(tex)), transform(m)
        {
        }
    };

    struct ImageAtlasData
    {
        std::shared_ptr<ITexture> texture;
        glm::mat3 transform{};
        AtlasRegion region;
        ImageAtlasData() = default;

        ImageAtlasData(std::shared_ptr<ITexture> tex, const glm::mat3& m, const AtlasRegion& r)
            : texture(std::move(tex)), transform(m), region(r)
        {
        }
    };

    struct TextData
    {
        std::wstring text;
        std::shared_ptr<ITextFormat> textFormat;
        Rect rect{};
        Color color{};
        Justification justification = Justification::Left;
        TextData() = default;

        TextData(std::wstring tx, std::shared_ptr<ITextFormat> fmt, const Rect& r, const Color& c,
                 Justification j = Justification::Left)
            : text(std::move(tx)), textFormat(std::move(fmt)), rect(r), color(c), justification(j)
        {
        }
    };

    struct RectangleData
    {
        Rect rect{};
        Color color{};
        float strokeWidth = 1.0f;
        bool filled = false;
        RectangleData() = default;

        RectangleData(const Rect& r, const Color& c, float sw, bool f)
            : rect(r), color(c), strokeWidth(sw), filled(f)
        {
        }
    };

    union ItemData
    {
        ImageData image;
        ImageAtlasData imageAtlas;
        TextData text;
        RectangleData rectangle;

        ItemData() { new(&image) ImageData(); }

        ~ItemData()
        {
        }
    };

    float opacity = 1.0f;
    int z = 0;
    size_t seq = 0;
    DrawType drawType = DrawType::Image;
    ItemData data;

    DrawItem() { new(&data.image) ImageData(); }
    ~DrawItem() { DestroyActive(); }

    DrawItem(const DrawItem& other);
    DrawItem(DrawItem&& other) noexcept;
    DrawItem& operator=(const DrawItem& other);
    DrawItem& operator=(DrawItem&& other) noexcept;

private:
    void DestroyActive();
};

class Renderer final
{
public:
    static bool Initialize(void* windowHandle);
    static void Resize(uint32_t width, uint32_t height);
    static void BeginFrame();
    static void Render();
    static void Cleanup();
    static void ToggleFPS();

    static void EnqueueImage(const std::shared_ptr<ITexture>& texture, const Transform& transform, float opacity,
                             int z);
    static void EnqueueReanim(const std::shared_ptr<ITexture>& texture, const ReanimatorTransform& transform, int z,
                              float opacity);
    static void EnqueueReanimAtlas(const std::shared_ptr<ITexture>& atlasTexture,
                                   const ReanimatorTransform& transform,
                                   const AtlasRegion& region,
                                   int z,
                                   float opacity);
    static void EnqueueTextW(const std::wstring& text,
                             const Rect& layoutRect,
                             const std::wstring& fontFamily,
                             float fontSize,
                             const Color& color,
                             int z,
                             Justification justification);
    static void EnqueueRectangle(const Rect& rect,
                                 const Color& color,
                                 float strokeWidth,
                                 bool filled,
                                 int z);

    static IRenderBackend* GetRenderBackend();

private:
    static void DrawFPS();
    static void FlushDrawQueue();

    static std::unique_ptr<IRenderBackend> backend_;
    static bool showFPS_;
    static std::vector<DrawItem> drawQueue_;
    static size_t submitSeq_;
};
