#include "SunObject.hpp"

#include <stdexcept>

SunObject::SunObject(int value) : ClickableObject(GameObjectTag::Sun), value_(value)
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
}

void SunObject::Render()
{
    ClickableObject::Render();

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
            .end = 0.0f,
            .setter = [this](float value)
            {
                transform_.position.y = value;
            },
            .mode = TweenMode::EaseIn
        });
    props.push_back({
        .start = transform_.position.x,
        .end = 0.0f,
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
