#include "Window.hpp"
#include "Input.hpp"
#include "../Render/Renderer.hpp"

#include <GLFW/glfw3.h>

#ifdef _WIN32
#define GLFW_EXPOSE_NATIVE_WIN32
#include <GLFW/glfw3native.h>
#endif

#include <iostream>

GLFWwindow* Window::window_ = nullptr;
uint32_t Window::width_ = 1280;
uint32_t Window::height_ = 720;

bool Window::Create(uint32_t width, uint32_t height, const char* title)
{
    if (!glfwInit())
    {
        std::cerr << "Failed to initialize GLFW\n";
        return false;
    }

    width_ = width;
    height_ = height;

    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
    glfwWindowHint(GLFW_VISIBLE, GLFW_TRUE);
    glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);

    window_ = glfwCreateWindow(static_cast<int>(width), static_cast<int>(height), title, nullptr, nullptr);
    if (!window_)
    {
        std::cerr << "Failed to create GLFW window\n";
        glfwTerminate();
        return false;
    }

    glfwSetKeyCallback(window_, KeyCallback);
    glfwSetMouseButtonCallback(window_, MouseButtonCallback);
    glfwSetCursorPosCallback(window_, MouseMoveCallback);
    glfwSetWindowSizeCallback(window_, WindowResizeCallback);

    return true;
}

void Window::Destroy()
{
    if (window_)
    {
        glfwDestroyWindow(window_);
        window_ = nullptr;
    }
    glfwTerminate();
}

bool Window::IsOpen()
{
    return window_ && !glfwWindowShouldClose(window_);
}

void Window::PollEvents()
{
    glfwPollEvents();
}

void Window::SwapBuffers()
{
    // glfwSwapBuffers(window_);
}

GLFWwindow* Window::GetNativeWindow()
{
    return window_;
}

void* Window::GetNativeWindowHandle()
{
#ifdef _WIN32
    if (window_)
    {
        return glfwGetWin32Window(window_);
    }
    return nullptr;
#elif defined(__APPLE__)
    if (window_)
    {
        return glfwGetCocoaWindow(window_);
    }
    return nullptr;
#else
    if (window_)
    {
        return glfwGetX11Window(window_);
    }
    return nullptr;
#endif
}

glm::vec2 Window::GetSize()
{
    return {width_, height_};
}

void Window::SetSize(uint32_t width, uint32_t height)
{
    if (window_)
    {
        glfwSetWindowSize(window_, static_cast<int>(width), static_cast<int>(height));
    }
}

glm::vec2 Window::GetMousePosition()
{
    double xpos, ypos;
    glfwGetCursorPos(window_, &xpos, &ypos);
    return {static_cast<float>(xpos), static_cast<float>(ypos)};
}

void Window::SetMousePosition(glm::vec2 position)
{
    if (window_)
    {
        glfwSetCursorPos(window_, position.x, position.y);
    }
}

static Key GlfwKeyToKey(int glfwKey)
{
    return static_cast<Key>(glfwKey);
}

static MouseButton GlfwMouseButtonToMouseButton(int glfwButton)
{
    return static_cast<MouseButton>(glfwButton);
}

void Window::KeyCallback(GLFWwindow* window, int key, int scancode, int action, int mods)
{
    const Key keyEnum = GlfwKeyToKey(key);

    if (action == GLFW_PRESS)
    {
        Input::HandleKeyDown(keyEnum);
        if (keyEnum == Key::F1)
        {
            Renderer::ToggleFPS();
        }
    }
    else if (action == GLFW_RELEASE)
    {
        Input::HandleKeyUp(keyEnum);
    }
}

void Window::MouseButtonCallback(GLFWwindow* window, int button, int action, int mods)
{
    const MouseButton mouseButton = GlfwMouseButtonToMouseButton(button);

    if (action == GLFW_PRESS)
    {
        Input::HandleMouseDown(mouseButton);
    }
    else if (action == GLFW_RELEASE)
    {
        Input::HandleMouseUp(mouseButton);
    }
}

void Window::MouseMoveCallback(GLFWwindow* window, double xpos, double ypos)
{
    Input::HandleMouseMove(glm::vec2(static_cast<float>(xpos), static_cast<float>(ypos)));
}

void Window::WindowResizeCallback(GLFWwindow* window, int width, int height)
{
    width_ = width;
    height_ = height;
    Renderer::Resize(static_cast<uint32_t>(width), static_cast<uint32_t>(height));
}
