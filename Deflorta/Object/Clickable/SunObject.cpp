#include "SunObject.hpp"

#include "../../Scene/BoardScene.hpp"
#include "../SeedBank.hpp"
#include "../../Base/Random.hpp"
#include "../../Base/Time.hpp"

#include <stdexcept>

SunObject::SunObject(glm::vec2 position, int value, SpawnAnimation spawnAnimation) : ClickableObject(
    position, GameObjectTag::Sun), value_(value)
{
    const auto reanim = ReanimationLoader::LoadFromFile("resources/reanim/Sun.xml");
    if (!reanim.has_value())
        throw std::runtime_error("Failed to load Sun reanim");

    sprite_ = std::make_unique<Reanimator>(reanim.value());
    sprite_->PlayLayer("Sun1", ReanimLoopType::Loop);
    sprite_->SetAllLayersZ(static_cast<int>(RenderLayer::Collectable));
    sprite_->SetAllLayersOpacity(0.0f);

    auto collider = std::make_unique<CircleCollider>(this, 30.0f);
    SetCollider(std::move(collider));

    disappearTimer_ = std::make_unique<Timer>();
    disappearTimer_->SetTimeoutCallback([this]
    {
        StartDisappearing();
    });
    disappearTimer_->SetOneShot(true);
    disappearTimer_->Start(20.0f);

    blinkTimer_ = std::make_unique<Timer>();
    blinkTimer_->SetTimeoutCallback([this]
    {
        StartBlinking();
    });
    blinkTimer_->SetOneShot(true);
    blinkTimer_->Start(16.0f);

    InitializeSpawnAnimation(spawnAnimation);
}

void SunObject::Render()
{
    if (sprite_)
        sprite_->Draw();
}

void SunObject::Update()
{
    ClickableObject::Update();

    if (sprite_)
    {
        sprite_->SetPosition(transform_.position);
        sprite_->Update();
    }

    blinkTimer_->Update();
    disappearTimer_->Update();

    if (spawnTween_)
    {
        spawnTween_->Update();
        if (!spawnTween_->IsActive())
        {
            spawnTween_.reset();
        }
    }

    if (spawnMoveTween_)
    {
        spawnMoveTween_->Update();
        if (!spawnMoveTween_->IsActive())
        {
            spawnMoveTween_.reset();
        }
    }

    if (blinkTween_)
    {
        blinkTween_->Update();
        if (!blinkTween_->IsActive())
        {
            if (isBlinkIn_)
            {
                std::vector<TweenProperty> props;
                props.push_back({
                    .start = 0.5f,
                    .end = 1.0f,
                    .setter = [this](float value)
                    {
                        if (sprite_)
                            sprite_->SetAllLayersOpacity(value);
                    },
                    .mode = TweenMode::EaseInOut
                });

                blinkTween_ = std::make_unique<Tween>(props, 0.5f);
                blinkTween_->Start();
                isBlinkIn_ = false;
            }
            else
            {
                std::vector<TweenProperty> props;
                props.push_back({
                    .start = 1.0f,
                    .end = 0.5f,
                    .setter = [this](float value)
                    {
                        if (sprite_)
                            sprite_->SetAllLayersOpacity(value);
                    },
                    .mode = TweenMode::EaseInOut
                });

                blinkTween_ = std::make_unique<Tween>(props, 0.5f);
                blinkTween_->Start();
                isBlinkIn_ = true;
            }
        }
    }

    if (moveTween_)
    {
        moveTween_->Update();
        if (!moveTween_->IsActive())
        {
            moveTween_.reset();
            StartDisappearing();
            if (const auto scene = dynamic_cast<BoardScene*>(GetScene()))
            {
                if (const auto seedBank = scene->GetSeedBank())
                {
                    seedBank->IncrementSunValue(value_);
                }
            }
        }
    }

    if (disappearTween_)
    {
        disappearTween_->Update();
        if (!disappearTween_->IsActive())
        {
            QueueFree();
        }
    }
}

void SunObject::OnMouseClick()
{
    collider_.reset();
    disappearTimer_->SetPaused(true);

    std::vector<TweenProperty> props;
    props.push_back(
        {
            .start = transform_.position.y,
            .end = 50.0f,
            .setter = [this](float value)
            {
                transform_.position.y = value;
            },
            .mode = TweenMode::EaseIn
        });
    props.push_back({
        .start = transform_.position.x,
        .end = 52.0f,
        .setter = [this](float value)
        {
            transform_.position.x = value;
        },
        .mode = TweenMode::EaseOut
    });

    moveTween_ = std::make_unique<Tween>(props, 1.0f);
    moveTween_->Start();
}

int SunObject::GetValue() const
{
    return value_;
}

void SunObject::StartBlinking()
{
    std::vector<TweenProperty> props;
    props.push_back({
        .start = 1.0f,
        .end = 0.5f,
        .setter = [this](float value)
        {
            if (sprite_)
                sprite_->SetAllLayersOpacity(value);
        },
        .mode = TweenMode::EaseInOut
    });

    blinkTween_ = std::make_unique<Tween>(props, 0.5f);
    blinkTween_->Start();
}

void SunObject::StartDisappearing()
{
    spawnMoveTween_.reset();
    blinkTween_.reset();
    std::vector<TweenProperty> props;
    props.push_back({
        .start = 1.0f,
        .end = 0.0f,
        .setter = [this](float value)
        {
            if (sprite_)
                sprite_->SetAllLayersOpacity(value);
        },
        .mode = TweenMode::EaseOut
    });

    disappearTween_ = std::make_unique<Tween>(props, 0.5f);
    disappearTween_->Start();
}

void SunObject::InitializeSpawnAnimation(SpawnAnimation spawnAnimation)
{
    switch (spawnAnimation)
    {
    case SpawnAnimation::None:
        {
            std::vector<TweenProperty> props;
            props.push_back({
                .start = 0.0f,
                .end = 1.0f,
                .setter = [this](float value)
                {
                    if (sprite_)
                        sprite_->SetAllLayersOpacity(value);
                },
                .mode = TweenMode::EaseOut
            });
            spawnTween_ = std::make_unique<Tween>(props, 0.5f);
            spawnTween_->Start();
            break;
        }
    case SpawnAnimation::DropDown:
        {
            sprite_->SetAllLayersOpacity(1.0f);

            constexpr float startY = -50.0f;
            const float endY = Random::UniformFloat(200.0f, 640.0f);

            std::vector<TweenProperty> props;
            props.push_back({
                .start = startY,
                .end = endY,
                .setter = [this](float value)
                {
                    transform_.position.y = value;
                },
                .mode = TweenMode::Linear
            });

            spawnMoveTween_ = std::make_unique<Tween>(props, 4.0f);
            spawnMoveTween_->Start();
            break;
        }
    case SpawnAnimation::Produce:

        float down = Random::UniformFloat(20.0f, 80.0f);
        down = transform_.position.y + down;

        float side = Random::UniformFloat(-70.0f, 70.0f);
        side = transform_.position.x + side;

        std::vector<TweenProperty> props;
        props.push_back({
            .start = transform_.position.y,
            .end = down,
            .setter = [this](float value)
            {
                transform_.position.y = value;
            },
            .mode = TweenMode::EaseIn
        });
        props.push_back({
            .start = transform_.position.x,
            .end = side,
            .setter = [this](float value)
            {
                transform_.position.x = value;
            },
            .mode = TweenMode::EaseOut
        });
        props.push_back({
            .start = 0.2f,
            .end = 1.0f,
            .setter = [this](float value)
            {
                if (sprite_)
                    sprite_->SetAllLayersOpacity(value);
            },
            .mode = TweenMode::EaseOut
        });

        spawnMoveTween_ = std::make_unique<Tween>(props, 1.0f);
        spawnMoveTween_->Start();
        break;
    }
}
