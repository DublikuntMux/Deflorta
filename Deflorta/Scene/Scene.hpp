#pragma once

class Scene
{
public:
    virtual ~Scene() = default;

    virtual void onEnter()
    {
    }

    virtual void onExit()
    {
    }

    virtual void update(float dt) = 0;
    virtual void render() = 0;
};
