#include "gamepad.h"

#include <SDL3/SDL.h>
#include <QDebug>

// GB button bits (match inputmap.cpp / core joypad).
enum { BitRight = 0, BitLeft = 1, BitUp = 2, BitDown = 3 };

// Stick-to-dpad hysteresis: press past 50%, release under 35%.
static constexpr Sint16 kPress = 16384;
static constexpr Sint16 kRelease = 11469;
static constexpr Sint16 kTriggerOn = 16384;
static constexpr Sint16 kTriggerOff = 8192;

// Rebindable actions, in settings-UI order (labels match InputMap's).
struct PadAction {
    const char *id;
    const char *label;
};
static const QList<PadAction> &padActions() {
    static const QList<PadAction> list = {
        {"a", "button a"},
        {"b", "button b"},
        {"start", "start"},
        {"select", "select"},
        {"turbo", "turbo (hold)"},
        {"pause", "pause"},
        {"save_state", "save state"},
        {"load_state", "load state"},
        {"next_slot", "next state slot"},
        {"screenshot", "screenshot"},
        {"reset", "reset game"},
        {"palette", "cycle palette"},
        {"mute", "mute"},
        {"fullscreen", "fullscreen"},
    };
    return list;
}

// inputId -> action. Physical Game Boy layout: A is the right button,
// B the lower one. Shoulder+trigger both turbo; lb and guide both pause.
static QHash<QString, QString> defaultBinds() {
    return {
        {"east", "a"},   {"south", "b"},
        {"start", "start"}, {"back", "select"},
        {"rb", "turbo"}, {"rt", "turbo"},
        {"lb", "pause"}, {"guide", "pause"},
    };
}

static const char *inputIdForButton(int button) {
    switch (button) {
    case SDL_GAMEPAD_BUTTON_SOUTH: return "south";
    case SDL_GAMEPAD_BUTTON_EAST: return "east";
    case SDL_GAMEPAD_BUTTON_WEST: return "west";
    case SDL_GAMEPAD_BUTTON_NORTH: return "north";
    case SDL_GAMEPAD_BUTTON_BACK: return "back";
    case SDL_GAMEPAD_BUTTON_GUIDE: return "guide";
    case SDL_GAMEPAD_BUTTON_START: return "start";
    case SDL_GAMEPAD_BUTTON_LEFT_STICK: return "l3";
    case SDL_GAMEPAD_BUTTON_RIGHT_STICK: return "r3";
    case SDL_GAMEPAD_BUTTON_LEFT_SHOULDER: return "lb";
    case SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER: return "rb";
    default: return nullptr;
    }
}

Gamepad::Gamepad(QObject *parent)
    : QObject(parent), m_settings("omaboy", "omaboy") {
    load();

    // Controllers keep working while another window has keyboard focus
    // (we own no SDL window at all).
    SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, "1");
    // Plain evdev only: SDL's HIDAPI drivers (libusb here) run blocking
    // device handshakes on hotplug, on this thread — 8BitDo pads froze
    // the UI. The kernel driver already exposes everything we need.
    SDL_SetHint(SDL_HINT_JOYSTICK_HIDAPI, "0");
    // SDL would otherwise swallow SIGINT/SIGTERM into SDL_EVENT_QUIT,
    // making the app unkillable from the terminal.
    SDL_SetHint(SDL_HINT_NO_SIGNAL_HANDLERS, "1");
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

// ---- bindings ----

void Gamepad::load() {
    m_binds.clear();
    m_settings.beginGroup("padBindings");
    const QStringList keys = m_settings.childKeys();
    for (const QString &k : keys)
        m_binds[k] = m_settings.value(k).toString();
    m_settings.endGroup();
    if (m_binds.isEmpty())
        m_binds = defaultBinds();
}

void Gamepad::save() {
    m_settings.beginGroup("padBindings");
    m_settings.remove("");
    for (auto it = m_binds.cbegin(); it != m_binds.cend(); ++it)
        m_settings.setValue(it.key(), it.value());
    m_settings.endGroup();
}

QVariantList Gamepad::model() const {
    QVariantList rows;
    for (const PadAction &a : padActions()) {
        QVariantMap row;
        row["id"] = a.id;
        row["label"] = a.label;
        row["padName"] = padName(a.id);
        rows.append(row);
    }
    return rows;
}

QString Gamepad::padName(const QString &action) const {
    QStringList inputs;
    for (auto it = m_binds.cbegin(); it != m_binds.cend(); ++it)
        if (it.value() == action)
            inputs.append(it.key());
    if (inputs.isEmpty())
        return QStringLiteral("—");
    inputs.sort();
    return inputs.join(QStringLiteral(" / "));
}

void Gamepad::rebind(const QString &action, const QString &inputId) {
    // One capture replaces the action's old inputs and steals the input
    // from whatever it was bound to (same auto-unbind as the keyboard).
    for (auto it = m_binds.begin(); it != m_binds.end();) {
        if (it.value() == action)
            it = m_binds.erase(it);
        else
            ++it;
    }
    m_binds[inputId] = action;
    save();
    emit changed();
}

void Gamepad::resetDefaults() {
    m_binds = defaultBinds();
    save();
    emit changed();
}

QString Gamepad::summary() const {
    QStringList parts;
    parts << QStringLiteral("stick/d-pad move");
    for (const PadAction &a : padActions()) {
        const QString inputs = padName(a.id);
        if (inputs != QStringLiteral("—"))
            parts << inputs + QStringLiteral("=") + a.id;
    }
    return parts.join(QStringLiteral(" · "));
}

void Gamepad::setCapturing(bool on) {
    if (m_capturing == on)
        return;
    m_capturing = on;
    emit capturingChanged();
}

// ---- device handling ----

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
    m_dpadMask = m_stickMask = 0;
    m_ltDown = m_rtDown = false;
    applyMask();
    const QSet<QString> held = m_held;
    m_held.clear();
    for (const QString &id : held) {
        const QString action = m_binds.value(id);
        if (!action.isEmpty())
            emit actionEvent(action, false);
    }
    emit connectionChanged();
}

void Gamepad::handleInput(const QString &inputId, bool down) {
    if (m_capturing) {
        if (down)
            emit captured(inputId);
        return;
    }
    if (down)
        m_held.insert(inputId);
    else if (!m_held.remove(inputId))
        return; // release for a press we never dispatched (e.g. capture)
    const QString action = m_binds.value(inputId);
    if (!action.isEmpty())
        emit actionEvent(action, down);
}

void Gamepad::setSourceBit(quint8 &mask, int bit, bool down) {
    if (down)
        mask |= quint8(1u << bit);
    else
        mask &= quint8(~(1u << bit));
}

void Gamepad::applyMask() {
    const quint8 now = quint8(m_dpadMask | m_stickMask);
    const quint8 diff = quint8(now ^ m_sentMask);
    if (!diff)
        return;
    for (int bit = 0; bit < 4; ++bit)
        if (diff & (1u << bit))
            emit buttonChanged(bit, now & (1u << bit));
    m_sentMask = now;
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
            case SDL_GAMEPAD_BUTTON_DPAD_UP:    setSourceBit(m_dpadMask, BitUp, down); applyMask(); break;
            case SDL_GAMEPAD_BUTTON_DPAD_DOWN:  setSourceBit(m_dpadMask, BitDown, down); applyMask(); break;
            case SDL_GAMEPAD_BUTTON_DPAD_LEFT:  setSourceBit(m_dpadMask, BitLeft, down); applyMask(); break;
            case SDL_GAMEPAD_BUTTON_DPAD_RIGHT: setSourceBit(m_dpadMask, BitRight, down); applyMask(); break;
            default:
                if (const char *id = inputIdForButton(ev.gbutton.button))
                    handleInput(QString::fromLatin1(id), down);
                break;
            }
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
                applyMask();
                break;
            case SDL_GAMEPAD_AXIS_LEFTY:
                if (v <= -kPress) setSourceBit(m_stickMask, BitUp, true);
                else if (v >= -kRelease) setSourceBit(m_stickMask, BitUp, false);
                if (v >= kPress) setSourceBit(m_stickMask, BitDown, true);
                else if (v <= kRelease) setSourceBit(m_stickMask, BitDown, false);
                applyMask();
                break;
            case SDL_GAMEPAD_AXIS_LEFT_TRIGGER:
                if (v >= kTriggerOn && !m_ltDown) { m_ltDown = true; handleInput("lt", true); }
                else if (v <= kTriggerOff && m_ltDown) { m_ltDown = false; handleInput("lt", false); }
                break;
            case SDL_GAMEPAD_AXIS_RIGHT_TRIGGER:
                if (v >= kTriggerOn && !m_rtDown) { m_rtDown = true; handleInput("rt", true); }
                else if (v <= kTriggerOff && m_rtDown) { m_rtDown = false; handleInput("rt", false); }
                break;
            default: break;
            }
            break;
        }

        default:
            break;
        }
    }
}
