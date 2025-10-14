#include "Foley.hpp"

std::vector<FoleyParams> Foley::FoleyParamArray = {
    {
        .foleyType = FoleyType::Sun,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_POINTS"}
    },
    {
        .foleyType = FoleyType::Splat,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_SPLAT", "SOUND_SPLAT2", "SOUND_SPLAT3"}
    },
    {
        .foleyType = FoleyType::Lawnmower,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_LAWNMOWER"}
    },
    {
        .foleyType = FoleyType::Throw,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_THROW", "SOUND_THROW", "SOUND_THROW", "SOUND_THROW2"}
    },
    {
        .foleyType = FoleyType::SpawnSun,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_THROW"}
    },
    {
        .foleyType = FoleyType::Chomp,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CHOMP", "SOUND_CHOMP2"}
    },
    {
        .foleyType = FoleyType::ChompSoft,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 4.0f,
        .sfxId = {"SOUND_CHOMPSOFT"}
    },
    {
        .foleyType = FoleyType::Plant,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PLANT", "SOUND_PLANT2"}
    },
    {
        .foleyType = FoleyType::UseShovel,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PLANT2"}
    },
    {
        .foleyType = FoleyType::Drop,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_TAP2"},
    },
    {
        .foleyType = FoleyType::Bleep,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_BLEEP"},
    },
    {
        .foleyType = FoleyType::Groan,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {
            "SOUND_GROAN",
            "SOUND_GROAN2",
            "SOUND_GROAN3",
            "SOUND_GROAN4",
            "SOUND_GROAN5",
            "SOUND_GROAN6"
        }
    },
    {
        .foleyType = FoleyType::Brains,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {
            "SOUND_GROAN",
            "SOUND_GROAN2",
            "SOUND_GROAN3",
            "SOUND_GROAN4",
            "SOUND_GROAN5",
            "SOUND_GROAN6",
            "SOUND_SUKHBIR4",
            "SOUND_SUKHBIR5",
            "SOUND_SUKHBIR6"
        }
    },
    {
        .foleyType = FoleyType::Sukhbir,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {
            "SOUND_GROAN",
            "SOUND_GROAN2",
            "SOUND_GROAN3",
            "SOUND_GROAN4",
            "SOUND_GROAN5",
            "SOUND_GROAN6",
            "SOUND_SUKHBIR",
            "SOUND_SUKHBIR2",
            "SOUND_SUKHBIR3"
        }
    },
    {
        .foleyType = FoleyType::JackInTheBox,
        .foleyFlags = FoleyFlags::Loop | FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_JACKINTHEBOX"}
    },
    {
        .foleyType = FoleyType::ArtChallenge,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_DIAMOND"}
    },
    {
        .foleyType = FoleyType::Zamboni,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_ZAMBONI"}
    },
    {
        .foleyType = FoleyType::Thunder,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_THUNDER"}
    },
    {
        .foleyType = FoleyType::Frozen,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_FROZEN"}
    },
    {
        .foleyType = FoleyType::ZombieSplash,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_PLANT_WATER", "SOUND_ZOMBIE_ENTERING_WATER"}
    },
    {
        .foleyType = FoleyType::BowlingImpact,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -3.0f,
        .sfxId = {"SOUND_BOWLINGIMPACT"}
    },
    {
        .foleyType = FoleyType::Squish,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CHOMP", "SOUND_CHOMP2"}
    },
    {
        .foleyType = FoleyType::TirePop,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_BALLOON_POP"}
    },
    {
        .foleyType = FoleyType::Explosion,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_EXPLOSION"}
    },
    {
        .foleyType = FoleyType::Slurp,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_SLURP"},
    },
    {
        .foleyType = FoleyType::LimbsPop,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_LIMBS_POP"}
    },
    {
        .foleyType = FoleyType::PogoZombie,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 4.0f,
        .sfxId = {"SOUND_POGO_ZOMBIE"}
    },
    {
        .foleyType = FoleyType::SnowPeaSparkles,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_SNOW_PEA_SPARKLES"}
    },
    {
        .foleyType = FoleyType::ZombieFalling,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_ZOMBIE_FALLING_1", "SOUND_ZOMBIE_FALLING_2"}
    },
    {
        .foleyType = FoleyType::Puff,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_PUFF"}
    },
    {
        .foleyType = FoleyType::Fume,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_FUME"}
    },
    {
        .foleyType = FoleyType::Coin,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_COIN"}
    },
    {
        .foleyType = FoleyType::KernelSplat,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_KERNELPULT", "SOUND_KERNELPULT2"}
    },
    {
        .foleyType = FoleyType::Digger,
        .foleyFlags = FoleyFlags::Loop | FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_DIGGER_ZOMBIE"}
    },
    {
        .foleyType = FoleyType::JackSurprise,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 1.0f,
        .sfxId = {"SOUND_JACK_SURPRISE", "SOUND_JACK_SURPRISE", "SOUND_JACK_SURPRISE2"}
    },
    {
        .foleyType = FoleyType::VaseBreaking,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -5.0f,
        .sfxId = {"SOUND_VASE_BREAKING"}
    },
    {
        .foleyType = FoleyType::PoolCleaner,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_POOL_CLEANER"},
    },
    {
        .foleyType = FoleyType::BasketBall,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_BASKETBALL"},
    },
    {
        .foleyType = FoleyType::Ignite,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_IGNITE", "SOUND_IGNITE", "SOUND_IGNITE", "SOUND_IGNITE2"}
    },
    {
        .foleyType = FoleyType::FirePea,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_FIREPEA"}
    },
    {
        .foleyType = FoleyType::Thumb,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_GARGANTUAR_THUMP"}
    },
    {
        .foleyType = FoleyType::SquashHmm,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_SQUASH_HMM", "SOUND_SQUASH_HMM", "SOUND_SQUASH_HMM2"}
    },
    {
        .foleyType = FoleyType::Magnetshroom,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_MAGNETSHROOM"}
    },
    {
        .foleyType = FoleyType::Butter,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_BUTTER"}
    },
    {
        .foleyType = FoleyType::BungeeScream,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_BUNGEE_SCREAM", "SOUND_BUNGEE_SCREAM2", "SOUND_BUNGEE_SCREAM3"}
    },
    {
        .foleyType = FoleyType::BossExplosionSmall,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_EXPLOSION"}
    },
    {
        .foleyType = FoleyType::ShieldHit,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_SHIELDHIT", "SOUND_SHIELDHIT2"}
    },
    {
        .foleyType = FoleyType::Swing,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_SWING"}
    },
    {
        .foleyType = FoleyType::Bonk,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_BONK"}
    },
    {
        .foleyType = FoleyType::Rain,
        .foleyFlags = FoleyFlags::Loop | FoleyFlags::MuteOnPause,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_RAIN"}
    },
    {
        .foleyType = FoleyType::DolphinBeforeJumping,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_DOLPHIN_BEFORE_JUMPING"}
    },
    {
        .foleyType = FoleyType::DolphinAppears,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_DOLPHIN_APPEARS"}
    },
    {
        .foleyType = FoleyType::PlantWater,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PLANT_WATER"}
    },
    {
        .foleyType = FoleyType::ZombieEnteringWater,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_ZOMBIE_ENTERING_WATER"}
    },
    {
        .foleyType = FoleyType::GravebusterChomp,
        .foleyFlags = FoleyFlags::MuteOnPause,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_GRAVEBUSTERCHOMP"}
    },
    {
        .foleyType = FoleyType::Cherrybomb,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CHERRYBOMB"}
    },
    {
        .foleyType = FoleyType::JalapenoIgnite,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_JALAPENO"}
    },
    {
        .foleyType = FoleyType::ReverseExplosion,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_REVERSE_EXPLOSION"}
    },
    {
        .foleyType = FoleyType::PlasticHit,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_PLASTICHIT", "SOUND_PLASTICHIT2"}
    },
    {
        .foleyType = FoleyType::WinMusic,
        .foleyFlags = FoleyFlags::MusicVolume,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_WINMUSIC"}
    },
    {
        .foleyType = FoleyType::BalloonInflate,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_BALLOONINFLATE"}
    },
    {
        .foleyType = FoleyType::BigChomp,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -2.0f,
        .sfxId = {"SOUND_BIGCHOMP"}
    },
    {
        .foleyType = FoleyType::MelonImpact,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -5.0f,
        .sfxId = {"SOUND_MELONIMPACT", "SOUND_MELONIMPACT2"}
    },
    {
        .foleyType = FoleyType::PlantGrow,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -2.0f,
        .sfxId = {"SOUND_PLANTGROW"}
    },
    {
        .foleyType = FoleyType::Shop,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -5.0f,
        .sfxId = {"SOUND_SHOOP"}
    },
    {
        .foleyType = FoleyType::Juicy,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_JUICY"}
    },
    {
        .foleyType = FoleyType::NewspaperRarrgh,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -2.0f,
        .sfxId = {"SOUND_NEWSPAPER_RARRGH", "SOUND_NEWSPAPER_RARRGH2", "SOUND_NEWSPAPER_RARRGH2"}
    },
    {
        .foleyType = FoleyType::NewspaperRip,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = -2.0f,
        .sfxId = {"SOUND_NEWSPAPER_RIP"}
    },
    {
        .foleyType = FoleyType::Floop,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_FLOOP"},
    },
    {
        .foleyType = FoleyType::Coffee,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_COFFEE"},
    },
    {
        .foleyType = FoleyType::LowGroan,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_LOWGROAN", "SOUND_LOWGROAN2"}
    },
    {
        .foleyType = FoleyType::Prize,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PRIZE"}
    },
    {
        .foleyType = FoleyType::Yuck,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 1.0f,
        .sfxId = {"SOUND_YUCK", "SOUND_YUCK", "SOUND_YUCK2"}
    },
    {
        .foleyType = FoleyType::Umbrella,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_THROW2"}
    },
    {
        .foleyType = FoleyType::GrassStep,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 2.0f,
        .sfxId = {"SOUND_GRASSSTEP"}
    },
    {
        .foleyType = FoleyType::Shovel,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_SHOVEL"}
    },
    {
        .foleyType = FoleyType::CobLaunch,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_COBLAUNCH"}
    },
    {
        .foleyType = FoleyType::Watering,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_WATERING"}
    },
    {
        .foleyType = FoleyType::Polevault,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_POLEVAULT"}
    },
    {
        .foleyType = FoleyType::GravestoneRumble,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 10.0f,
        .sfxId = {"SOUND_GRAVESTONE_RUMBLE"}
    },
    {
        .foleyType = FoleyType::DirtRise,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_DIRT_RISE"}
    },
    {
        .foleyType = FoleyType::Fertilizer,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_FERTILIZER"}
    },
    {
        .foleyType = FoleyType::Portal,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PORTAL"}
    },
    {
        .foleyType = FoleyType::Wakeup,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_WAKEUP"}
    },
    {
        .foleyType = FoleyType::BugSpray,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_BUGSPRAY"}
    },
    {
        .foleyType = FoleyType::Scream,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_SCREAM"}
    },
    {
        .foleyType = FoleyType::Paper,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PAPER"}
    },
    {
        .foleyType = FoleyType::MoneyFalls,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_MONEYFALLS"}
    },
    {
        .foleyType = FoleyType::Imp,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 5.0f,
        .sfxId = {"SOUND_IMP", "SOUND_IMP2"}
    },
    {
        .foleyType = FoleyType::HydraulicShort,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 3.0f,
        .sfxId = {"SOUND_HYDRAULIC_SHORT"}
    },
    {
        .foleyType = FoleyType::Hydraulic,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_HYDRAULIC"}
    },
    {
        .foleyType = FoleyType::GargantuaDeath,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 3.0f,
        .sfxId = {"SOUND_GARGANTUDEATH"}
    },
    {
        .foleyType = FoleyType::Ceramic,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CERAMIC"}
    },
    {
        .foleyType = FoleyType::BossBoulderAttack,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_BOSSBOULDERATTACK"}
    },
    {
        .foleyType = FoleyType::Chime,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CHIME"}
    },
    {
        .foleyType = FoleyType::CrazyDaveShort,
        .foleyFlags = FoleyFlags::DontRepeat,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVESHORT1", "SOUND_CRAZYDAVESHORT2", "SOUND_CRAZYDAVESHORT3"}
    },
    {
        .foleyType = FoleyType::CrazyDaveLong,
        .foleyFlags = FoleyFlags::DontRepeat,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVELONG1", "SOUND_CRAZYDAVELONG2", "SOUND_CRAZYDAVELONG3"},
    },
    {
        .foleyType = FoleyType::CrazyDaveExtraLong,
        .foleyFlags = FoleyFlags::DontRepeat,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVEEXTRALONG1", "SOUND_CRAZYDAVEEXTRALONG2", "SOUND_CRAZYDAVEEXTRALONG3"}
    },
    {
        .foleyType = FoleyType::CrazyDaveCrazy,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVECRAZY"}
    },
    {
        .foleyType = FoleyType::Phonograph,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_PHONOGRAPH"}
    },
    {
        .foleyType = FoleyType::Dancer,
        .foleyFlags = FoleyFlags::OneAtTime | FoleyFlags::MuteOnPause,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_DANCER"}
    },
    {
        .foleyType = FoleyType::FinalFanfare,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_FINALFANFARE"}
    },
    {
        .foleyType = FoleyType::CrazyDaveScream,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVESCREAM"},
    },
    {
        .foleyType = FoleyType::CrazyDaveScream2,
        .foleyFlags = FoleyFlags::None,
        .pitchRange = 0.0f,
        .sfxId = {"SOUND_CRAZYDAVESCREAM2"}
    }
};
