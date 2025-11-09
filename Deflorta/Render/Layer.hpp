#pragma once

enum class RenderLayer: int
{
    Background = -1,
    BackgroundCover,
    Default,
    PlantUnder,
    PlantBase,
    PlantCover,
    PlantTop,
    Zombie,
    Projectile,
    Foreground,
    Fog,
    UI,
    Collectable,
    Debug,
    Count
};
