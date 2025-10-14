#pragma once

#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>

enum class FoleyFlags: std::uint8_t
{
    None = 0,
    Loop = 1 << 0,
    OneAtTime = 1 << 1,
    MuteOnPause = 1 << 2,
    MusicVolume = 1 << 3,
    DontRepeat = 1 << 4,
};

inline FoleyFlags operator|(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) | static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator&(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) & static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator^(FoleyFlags lhs, FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        static_cast<uint8_t>(lhs) ^ static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags operator~(FoleyFlags rhs)
{
    return static_cast<FoleyFlags>(
        ~static_cast<uint8_t>(rhs)
    );
}

inline FoleyFlags& operator|=(FoleyFlags& lhs, FoleyFlags rhs)
{
    lhs = lhs | rhs;
    return lhs;
}

inline FoleyFlags& operator&=(FoleyFlags& lhs, FoleyFlags rhs)
{
    lhs = lhs & rhs;
    return lhs;
}


enum class FoleyType: std::uint8_t
{
    Sun,
    Splat,
    Lawnmower,
    Throw,
    SpawnSun,
    Chomp,
    ChompSoft,
    Plant,
    UseShovel,
    Drop,
    Bleep,
    Groan,
    Brains,
    Sukhbir,
    JackInTheBox,
    ArtChallenge,
    Zamboni,
    Thunder,
    Frozen,
    ZombieSplash,
    BowlingImpact,
    Squish,
    TirePop,
    Explosion,
    Slurp,
    LimbsPop,
    PogoZombie,
    SnowPeaSparkles,
    ZombieFalling,
    Puff,
    Fume,
    Coin,
    KernelSplat,
    Digger,
    JackSurprise,
    VaseBreaking,
    PoolCleaner,
    BasketBall,
    Ignite,
    FirePea,
    Thumb,
    SquashHmm,
    Magnetshroom,
    Butter,
    BungeeScream,
    BossExplosionSmall,
    ShieldHit,
    Swing,
    Bonk,
    Rain,
    DolphinBeforeJumping,
    DolphinAppears,
    PlantWater,
    ZombieEnteringWater,
    GravebusterChomp,
    Cherrybomb,
    JalapenoIgnite,
    ReverseExplosion,
    PlasticHit,
    WinMusic,
    BalloonInflate,
    BigChomp,
    MelonImpact,
    PlantGrow,
    Shop,
    Juicy,
    NewspaperRarrgh,
    NewspaperRip,
    Floop,
    Coffee,
    LowGroan,
    Prize,
    Yuck,
    Umbrella,
    GrassStep,
    Shovel,
    CobLaunch,
    Watering,
    Polevault,
    GravestoneRumble,
    DirtRise,
    Fertilizer,
    Portal,
    Wakeup,
    BugSpray,
    Scream,
    Paper,
    MoneyFalls,
    Imp,
    HydraulicShort,
    Hydraulic,
    GargantuaDeath,
    Ceramic,
    BossBoulderAttack,
    Chime,
    CrazyDaveShort,
    CrazyDaveLong,
    CrazyDaveExtraLong,
    CrazyDaveCrazy,
    Phonograph,
    Dancer,
    FinalFanfare,
    CrazyDaveScream,
    CrazyDaveScream2
};

struct FoleyParams
{
    FoleyFlags foleyFlags;
    float pitchRange;
    std::vector<std::string> sfxId;
};

class Foley
{
public:
    static const std::unordered_map<FoleyType, FoleyParams> foleyMap_;
};
