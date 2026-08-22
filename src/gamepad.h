// Game controller input via SDL3: hotplug, d-pad + left stick merged
// onto the GB d-pad, and rebindable button-to-action mappings persisted
// in QSettings (mirrors InputMap for the keyboard).
#pragma once

#include <QHash>
#include <QObject>
#include <QSet>
#include <QSettings>
#include <QTimer>
#include <QVariantList>

struct SDL_Gamepad;

class Gamepad : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool connected READ connected NOTIFY connectionChanged)
    Q_PROPERTY(QString name READ name NOTIFY connectionChanged)
    Q_PROPERTY(bool capturing READ capturing WRITE setCapturing NOTIFY capturingChanged)

public:
    explicit Gamepad(QObject *parent = nullptr);
    ~Gamepad() override;

    bool connected() const { return m_pad != nullptr; }
    QString name() const { return m_name; }
    bool capturing() const { return m_capturing; }
    void setCapturing(bool on);

    /// Rows for the settings UI: {id, label, padName}.
    Q_INVOKABLE QVariantList model() const;

    /// Pad inputs bound to an action, e.g. "east" or "rb / rt", or "—".
    /// Face buttons use the connected pad's printed labels when known.
    Q_INVOKABLE QString padName(const QString &action) const;

    /// Bind a captured input to an action (replaces both sides' bindings).
    Q_INVOKABLE void rebind(const QString &action, const QString &inputId);
    Q_INVOKABLE void resetDefaults();

    /// One-line binding summary for the help overlay.
    Q_INVOKABLE QString summary() const;

signals:
    /// GB d-pad bit (0=Right..3=Down) from the pad's d-pad or left stick.
    void buttonChanged(int bit, bool down);
    /// A bound action (a/b/start/select/turbo/pause/...) pressed/released.
    void actionEvent(const QString &action, bool down);
    /// Capture mode: the input the user just pressed ("south", "rt", ...).
    void captured(const QString &inputId);
    void connectionChanged();
    void capturingChanged();
    void changed(); // bindings
    void toast(const QString &message);

private:
    void poll();
    void openFirstAvailable();
    void closePad();
    QString inputDisplayName(const QString &inputId) const;
    void handleInput(const QString &inputId, bool down);
    void setSourceBit(quint8 &mask, int bit, bool down);
    void applyMask();
    void load();
    void save();

    SDL_Gamepad *m_pad = nullptr;
    quint32 m_instanceId = 0;
    QString m_name;
    QTimer m_timer;
    QSettings m_settings;

    QHash<QString, QString> m_binds; // inputId -> action
    QSet<QString> m_held;            // inputs currently down (for release on unplug)
    bool m_capturing = false;

    // D-pad and stick each keep their own view of bits 0..3; the core
    // sees the OR so releasing one source never drops the other's hold.
    quint8 m_dpadMask = 0;
    quint8 m_stickMask = 0;
    quint8 m_sentMask = 0;
    bool m_ltDown = false;
    bool m_rtDown = false;
    bool m_sdlReady = false;
};
