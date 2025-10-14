#include "Foley.hpp"

const std::unordered_map<FoleyType, FoleyParams> Foley::foleyMap_ = {
    {FoleyType::Sun, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_POINTS"}}},
    {
        FoleyType::Splat,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_SPLAT", "SOUND_SPLAT2", "SOUND_SPLAT3"}}
    },
    {FoleyType::Lawnmower, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_LAWNMOWER"}}},
    {
        FoleyType::Throw,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 10.0f,
            .sfxId = {"SOUND_THROW", "SOUND_THROW", "SOUND_THROW", "SOUND_THROW2"}
        }
    },
    {FoleyType::SpawnSun, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_THROW"}}},
    {FoleyType::Chomp, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CHOMP", "SOUND_CHOMP2"}}},
    {FoleyType::ChompSoft, {.foleyFlags = FoleyFlags::None, .pitchRange = 4.0f, .sfxId = {"SOUND_CHOMPSOFT"}}},
    {FoleyType::Plant, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PLANT", "SOUND_PLANT2"}}},
    {FoleyType::UseShovel, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PLANT2"}}},
    {FoleyType::Drop, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_TAP2"}}},
    {FoleyType::Bleep, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_BLEEP"}}},
    {
        FoleyType::Groan,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 0.0f,
            .sfxId = {"SOUND_GROAN", "SOUND_GROAN2", "SOUND_GROAN3", "SOUND_GROAN4", "SOUND_GROAN5", "SOUND_GROAN6"}
        }
    },
    {
        FoleyType::Brains,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 0.0f,
            .sfxId = {
                "SOUND_GROAN", "SOUND_GROAN2", "SOUND_GROAN3", "SOUND_GROAN4", "SOUND_GROAN5", "SOUND_GROAN6",
                "SOUND_SUKHBIR4", "SOUND_SUKHBIR5", "SOUND_SUKHBIR6"
            }
        }
    },
    {
        FoleyType::Sukhbir,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 0.0f,
            .sfxId = {
                "SOUND_GROAN", "SOUND_GROAN2", "SOUND_GROAN3", "SOUND_GROAN4", "SOUND_GROAN5", "SOUND_GROAN6",
                "SOUND_SUKHBIR", "SOUND_SUKHBIR2", "SOUND_SUKHBIR3"
            }
        }
    },
    {
        FoleyType::JackInTheBox, {
            .foleyFlags = FoleyFlags::Loop | FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause,
            .pitchRange = 0.0f, .sfxId = {"SOUND_JACKINTHEBOX"}
        }
    },
    {FoleyType::ArtChallenge, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_DIAMOND"}}},
    {FoleyType::Zamboni, {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_ZAMBONI"}}},
    {FoleyType::Thunder, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_THUNDER"}}},
    {FoleyType::Frozen, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_FROZEN"}}},
    {
        FoleyType::ZombieSplash,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 10.0f,
            .sfxId = {"SOUND_PLANT_WATER", "SOUND_ZOMBIE_ENTERING_WATER"}
        }
    },
    {FoleyType::BowlingImpact, {.foleyFlags = FoleyFlags::None, .pitchRange = -3.0f, .sfxId = {"SOUND_BOWLINGIMPACT"}}},
    {FoleyType::Squish, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CHOMP", "SOUND_CHOMP2"}}},
    {FoleyType::TirePop, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_BALLOON_POP"}}},
    {FoleyType::Explosion, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_EXPLOSION"}}},
    {FoleyType::Slurp, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_SLURP"}}},
    {FoleyType::LimbsPop, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_LIMBS_POP"}}},
    {FoleyType::PogoZombie, {.foleyFlags = FoleyFlags::None, .pitchRange = 4.0f, .sfxId = {"SOUND_POGO_ZOMBIE"}}},
    {
        FoleyType::SnowPeaSparkles,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_SNOW_PEA_SPARKLES"}}
    },
    {
        FoleyType::ZombieFalling,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 10.0f,
            .sfxId = {"SOUND_ZOMBIE_FALLING_1", "SOUND_ZOMBIE_FALLING_2"}
        }
    },
    {FoleyType::Puff, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_PUFF"}}},
    {FoleyType::Fume, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_FUME"}}},
    {FoleyType::Coin, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_COIN"}}},
    {
        FoleyType::KernelSplat,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_KERNELPULT", "SOUND_KERNELPULT2"}}
    },
    {
        FoleyType::Digger, {
            .foleyFlags = FoleyFlags::Loop | FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause,
            .pitchRange = 0.0f, .sfxId = {"SOUND_DIGGER_ZOMBIE"}
        }
    },
    {
        FoleyType::JackSurprise,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 1.0f,
            .sfxId = {"SOUND_JACK_SURPRISE", "SOUND_JACK_SURPRISE", "SOUND_JACK_SURPRISE2"}
        }
    },
    {FoleyType::VaseBreaking, {.foleyFlags = FoleyFlags::None, .pitchRange = -5.0f, .sfxId = {"SOUND_VASE_BREAKING"}}},
    {FoleyType::PoolCleaner, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_POOL_CLEANER"}}},
    {FoleyType::BasketBall, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_BASKETBALL"}}},
    {
        FoleyType::Ignite,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 5.0f,
            .sfxId = {"SOUND_IGNITE", "SOUND_IGNITE", "SOUND_IGNITE", "SOUND_IGNITE2"}
        }
    },
    {FoleyType::FirePea, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_FIREPEA"}}},
    {FoleyType::Thumb, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_GARGANTUAR_THUMP"}}},
    {
        FoleyType::SquashHmm,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 2.0f,
            .sfxId = {"SOUND_SQUASH_HMM", "SOUND_SQUASH_HMM", "SOUND_SQUASH_HMM2"}
        }
    },
    {FoleyType::Magnetshroom, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_MAGNETSHROOM"}}},
    {FoleyType::Butter, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_BUTTER"}}},
    {
        FoleyType::BungeeScream,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = 2.0f,
            .sfxId = {"SOUND_BUNGEE_SCREAM", "SOUND_BUNGEE_SCREAM2", "SOUND_BUNGEE_SCREAM3"}
        }
    },
    {FoleyType::BossExplosionSmall, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_EXPLOSION"}}},
    {
        FoleyType::ShieldHit,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_SHIELDHIT", "SOUND_SHIELDHIT2"}}
    },
    {FoleyType::Swing, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_SWING"}}},
    {FoleyType::Bonk, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_BONK"}}},
    {
        FoleyType::Rain,
        {.foleyFlags = FoleyFlags::Loop | FoleyFlags::MuteOnPause, .pitchRange = 0.0f, .sfxId = {"SOUND_RAIN"}}
    },
    {
        FoleyType::DolphinBeforeJumping,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_DOLPHIN_BEFORE_JUMPING"}}
    },
    {
        FoleyType::DolphinAppears,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_DOLPHIN_APPEARS"}}
    },
    {FoleyType::PlantWater, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PLANT_WATER"}}},
    {
        FoleyType::ZombieEnteringWater,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_ZOMBIE_ENTERING_WATER"}}
    },
    {
        FoleyType::GravebusterChomp,
        {.foleyFlags = FoleyFlags::MuteOnPause, .pitchRange = 0.0f, .sfxId = {"SOUND_GRAVEBUSTERCHOMP"}}
    },
    {FoleyType::Cherrybomb, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CHERRYBOMB"}}},
    {FoleyType::JalapenoIgnite, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_JALAPENO"}}},
    {
        FoleyType::ReverseExplosion,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_REVERSE_EXPLOSION"}}
    },
    {
        FoleyType::PlasticHit,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_PLASTICHIT", "SOUND_PLASTICHIT2"}}
    },
    {FoleyType::WinMusic, {.foleyFlags = FoleyFlags::MusicVolume, .pitchRange = 0.0f, .sfxId = {"SOUND_WINMUSIC"}}},
    {
        FoleyType::BalloonInflate,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_BALLOONINFLATE"}}
    },
    {FoleyType::BigChomp, {.foleyFlags = FoleyFlags::None, .pitchRange = -2.0f, .sfxId = {"SOUND_BIGCHOMP"}}},
    {
        FoleyType::MelonImpact,
        {.foleyFlags = FoleyFlags::None, .pitchRange = -5.0f, .sfxId = {"SOUND_MELONIMPACT", "SOUND_MELONIMPACT2"}}
    },
    {FoleyType::PlantGrow, {.foleyFlags = FoleyFlags::None, .pitchRange = -2.0f, .sfxId = {"SOUND_PLANTGROW"}}},
    {FoleyType::Shop, {.foleyFlags = FoleyFlags::None, .pitchRange = -5.0f, .sfxId = {"SOUND_SHOOP"}}},
    {FoleyType::Juicy, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_JUICY"}}},
    {
        FoleyType::NewspaperRarrgh,
        {
            .foleyFlags = FoleyFlags::None, .pitchRange = -2.0f,
            .sfxId = {"SOUND_NEWSPAPER_RARRGH", "SOUND_NEWSPAPER_RARRGH2", "SOUND_NEWSPAPER_RARRGH2"}
        }
    },
    {FoleyType::NewspaperRip, {.foleyFlags = FoleyFlags::None, .pitchRange = -2.0f, .sfxId = {"SOUND_NEWSPAPER_RIP"}}},
    {FoleyType::Floop, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_FLOOP"}}},
    {FoleyType::Coffee, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_COFFEE"}}},
    {
        FoleyType::LowGroan,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_LOWGROAN", "SOUND_LOWGROAN2"}}
    },
    {FoleyType::Prize, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PRIZE"}}},
    {
        FoleyType::Yuck,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 1.0f, .sfxId = {"SOUND_YUCK", "SOUND_YUCK", "SOUND_YUCK2"}}
    },
    {FoleyType::Umbrella, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_THROW2"}}},
    {FoleyType::GrassStep, {.foleyFlags = FoleyFlags::None, .pitchRange = 2.0f, .sfxId = {"SOUND_GRASSSTEP"}}},
    {FoleyType::Shovel, {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_SHOVEL"}}},
    {FoleyType::CobLaunch, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_COBLAUNCH"}}},
    {FoleyType::Watering, {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_WATERING"}}},
    {FoleyType::Polevault, {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_POLEVAULT"}}},
    {
        FoleyType::GravestoneRumble,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 10.0f, .sfxId = {"SOUND_GRAVESTONE_RUMBLE"}}
    },
    {FoleyType::DirtRise, {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_DIRT_RISE"}}},
    {FoleyType::Fertilizer, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_FERTILIZER"}}},
    {FoleyType::Portal, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PORTAL"}}},
    {FoleyType::Wakeup, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_WAKEUP"}}},
    {FoleyType::BugSpray, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_BUGSPRAY"}}},
    {FoleyType::Scream, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_SCREAM"}}},
    {FoleyType::Paper, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PAPER"}}},
    {FoleyType::MoneyFalls, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_MONEYFALLS"}}},
    {FoleyType::Imp, {.foleyFlags = FoleyFlags::None, .pitchRange = 5.0f, .sfxId = {"SOUND_IMP", "SOUND_IMP2"}}},
    {
        FoleyType::HydraulicShort,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 3.0f, .sfxId = {"SOUND_HYDRAULIC_SHORT"}}
    },
    {FoleyType::Hydraulic, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_HYDRAULIC"}}},
    {FoleyType::GargantuaDeath, {.foleyFlags = FoleyFlags::None, .pitchRange = 3.0f, .sfxId = {"SOUND_GARGANTUDEATH"}}},
    {FoleyType::Ceramic, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CERAMIC"}}},
    {
        FoleyType::BossBoulderAttack,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_BOSSBOULDERATTACK"}}
    },
    {FoleyType::Chime, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CHIME"}}},
    {
        FoleyType::CrazyDaveShort,
        {
            .foleyFlags = FoleyFlags::DontRepeat, .pitchRange = 0.0f,
            .sfxId = {"SOUND_CRAZYDAVESHORT1", "SOUND_CRAZYDAVESHORT2", "SOUND_CRAZYDAVESHORT3"}
        }
    },
    {
        FoleyType::CrazyDaveLong,
        {
            .foleyFlags = FoleyFlags::DontRepeat, .pitchRange = 0.0f,
            .sfxId = {"SOUND_CRAZYDAVELONG1", "SOUND_CRAZYDAVELONG2", "SOUND_CRAZYDAVELONG3"}
        }
    },
    {
        FoleyType::CrazyDaveExtraLong,
        {
            .foleyFlags = FoleyFlags::DontRepeat, .pitchRange = 0.0f,
            .sfxId = {"SOUND_CRAZYDAVEEXTRALONG1", "SOUND_CRAZYDAVEEXTRALONG2", "SOUND_CRAZYDAVEEXTRALONG3"}
        }
    },
    {
        FoleyType::CrazyDaveCrazy,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CRAZYDAVECRAZY"}}
    },
    {FoleyType::Phonograph, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_PHONOGRAPH"}}},
    {
        FoleyType::Dancer, {
            .foleyFlags = FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause, .pitchRange = 0.0f,
            .sfxId = {"SOUND_DANCER"}
        }
    },
    {FoleyType::FinalFanfare, {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_FINALFANFARE"}}},
    {
        FoleyType::CrazyDaveScream,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CRAZYDAVESCREAM"}}
    },
    {
        FoleyType::CrazyDaveScream2,
        {.foleyFlags = FoleyFlags::None, .pitchRange = 0.0f, .sfxId = {"SOUND_CRAZYDAVESCREAM2"}}
    }
};
