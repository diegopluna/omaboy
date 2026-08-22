#include "gamepad.h"

#include <SDL3/SDL.h>
#include <QDebug>

// GB button bits (match inputmap.cpp / core joypad).
enum { BitRight = 0, BitLeft = 1, BitUp = 2, BitDown = 3,
       BitA = 4, BitB = 5, BitSelect = 6, BitStart = 7 };

// Stick-to-dpad hysteresis: press past 50%, release under 35%.
static constexpr Sint16 kPress = 16384;
static constexpr Sint16 kRelease = 11469;
static constexpr Sint16 kTriggerOn = 16384;
static constexpr Sint16 kTriggerOff = 8192;

Gamepad::Gamepad(QObject *parent) : QObject(parent) {
    // Controllers keep working while another window has keyboard focus
    // (we own no SDL window at all).
    SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, "1");
    if (!SDL_InitSubSystem(SDL_INIT_GAMEPAD)) {
        qWarning() << "gamepad: SDL init failed:" << SDL_GetError();
        return;
    }
    m_sdlReady = true;
    openFirstAvailable();

    m_timer.setInterval(8);
    m_timer.setTimerType(Qt::PreciseTimer);
    connect(&m_timer, &QTimer::timeout, this, &Gamepad::poll);
    m_timer.start();
}

Gamepad::~Gamepad() {
    closePad();
    if (m_sdlReady)
        SDL_QuitSubSystem(SDL_INIT_GAMEPAD);
}

void Gamepad::openFirstAvailable() {
    int count = 0;
    SDL_JoystickID *ids = SDL_GetGamepads(&count);
    if (ids && count > 0) {
        m_pad = SDL_OpenGamepad(ids[0]);
        if (m_pad) {
            m_instanceId = ids[0];
            m_name = QString::fromUtf8(SDL_GetGamepadName(m_pad));
            emit connectionChanged();
            emit toast("controller · " + m_name.toLower());
        }
    }
    SDL_free(ids);
}

void Gamepad::closePad() {
    if (!m_pad)
        return;
    SDL_CloseGamepad(m_pad);
    m_pad = nullptr;
    m_instanceId = 0;
    m_name.clear();
    // Release everything the pad was holding.
    m_dpadMask = m_stickMask = m_buttonMask = 0;
    m_shoulderTurbo = m_triggerTurbo = false;
    applyMask();
    emit connectionChanged();
}

void Gamepad::setSourceBit(quint8 &mask, int bit, bool down) {
    if (down)
        mask |= quint8(1u << bit);
    else
        mask &= quint8(~(1u << bit));
}

void Gamepad::applyMask() {
    const quint8 now = quint8(m_dpadMask | m_stickMask | m_buttonMask);
    const quint8 diff = quint8(now ^ m_sentMask);
    if (diff) {
        for (int bit = 0; bit < 8; ++bit)
            if (diff & (1u << bit))
                emit buttonChanged(bit, now & (1u << bit));
        m_sentMask = now;
    }
    const bool turbo = m_shoulderTurbo || m_triggerTurbo;
    if (turbo != m_sentTurbo) {
        m_sentTurbo = turbo;
        emit turboChanged(turbo);
    }
}

void Gamepad::poll() {
    SDL_Event ev;
    while (SDL_PollEvent(&ev)) {
        switch (ev.type) {
        case SDL_EVENT_GAMEPAD_ADDED:
            if (!m_pad) {
                m_pad = SDL_OpenGamepad(ev.gdevice.which);
                if (m_pad) {
                    m_instanceId = ev.gdevice.which;
                    m_name = QString::fromUtf8(SDL_GetGamepadName(m_pad));
                    emit connectionChanged();
                    emit toast("controller · " + m_name.toLower());
                }
            }
            break;

        case SDL_EVENT_GAMEPAD_REMOVED:
            if (m_pad && ev.gdevice.which == m_instanceId) {
                emit toast("controller disconnected");
                closePad();
                openFirstAvailable(); // fall back to another pad if present
            }
            break;

        case SDL_EVENT_GAMEPAD_BUTTON_DOWN:
        case SDL_EVENT_GAMEPAD_BUTTON_UP: {
            if (!m_pad || ev.gbutton.which != m_instanceId)
                break;
            const bool down = ev.gbutton.down;
            switch (ev.gbutton.button) {
            case SDL_GAMEPAD_BUTTON_DPAD_UP:    setSourceBit(m_dpadMask, BitUp, down); break;
            case SDL_GAMEPAD_BUTTON_DPAD_DOWN:  setSourceBit(m_dpadMask, BitDown, down); break;
            case SDL_GAMEPAD_BUTTON_DPAD_LEFT:  setSourceBit(m_dpadMask, BitLeft, down); break;
            case SDL_GAMEPAD_BUTTON_DPAD_RIGHT: setSourceBit(m_dpadMask, BitRight, down); break;
            // Physical Game Boy layout: A is the right button, B the lower-left.
            case SDL_GAMEPAD_BUTTON_EAST:  setSourceBit(m_buttonMask, BitA, down); break;
            case SDL_GAMEPAD_BUTTON_SOUTH: setSourceBit(m_buttonMask, BitB, down); break;
            case SDL_GAMEPAD_BUTTON_START: setSourceBit(m_buttonMask, BitStart, down); break;
            case SDL_GAMEPAD_BUTTON_BACK:  setSourceBit(m_buttonMask, BitSelect, down); break;
            case SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER: m_shoulderTurbo = down; break;
            case SDL_GAMEPAD_BUTTON_LEFT_SHOULDER:
            case SDL_GAMEPAD_BUTTON_GUIDE:
                if (down)
                    emit pausePressed();
                break;
            default: break;
            }
            applyMask();
            break;
        }

        case SDL_EVENT_GAMEPAD_AXIS_MOTION: {
            if (!m_pad || ev.gaxis.which != m_instanceId)
                break;
            const Sint16 v = ev.gaxis.value;
            switch (ev.gaxis.axis) {
            case SDL_GAMEPAD_AXIS_LEFTX:
                if (v <= -kPress) setSourceBit(m_stickMask, BitLeft, true);
                else if (v >= -kRelease) setSourceBit(m_stickMask, BitLeft, false);
                if (v >= kPress) setSourceBit(m_stickMask, BitRight, true);
                else if (v <= kRelease) setSourceBit(m_stickMask, BitRight, false);
                break;
            case SDL_GAMEPAD_AXIS_LEFTY:
                if (v <= -kPress) setSourceBit(m_stickMask, BitUp, true);
                else if (v >= -kRelease) setSourceBit(m_stickMask, BitUp, false);
                if (v >= kPress) setSourceBit(m_stickMask, BitDown, true);
                else if (v <= kRelease) setSourceBit(m_stickMask, BitDown, false);
                break;
            case SDL_GAMEPAD_AXIS_RIGHT_TRIGGER:
                if (v >= kTriggerOn) m_triggerTurbo = true;
                else if (v <= kTriggerOff) m_triggerTurbo = false;
                break;
            default: break;
            }
            applyMask();
            break;
        }

        default:
            break;
        }
    }
}
