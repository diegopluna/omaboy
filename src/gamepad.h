// Game controller input via SDL3: hotplug, d-pad + left stick merged,
// fixed mapping onto the same button bits / actions the keyboard uses.
#pragma once

#include <QObject>
#include <QTimer>

struct SDL_Gamepad;

class Gamepad : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool connected READ connected NOTIFY connectionChanged)
    Q_PROPERTY(QString name READ name NOTIFY connectionChanged)

public:
    explicit Gamepad(QObject *parent = nullptr);
    ~Gamepad() override;

    bool connected() const { return m_pad != nullptr; }
    QString name() const { return m_name; }

signals:
    /// GB button bit (0=Right..7=Start) pressed or released.
    void buttonChanged(int bit, bool down);
    void turboChanged(bool down);
    void pausePressed();
    void connectionChanged();
    void toast(const QString &message);

private:
    void poll();
    void openFirstAvailable();
    void closePad();
    void setSourceBit(quint8 &mask, int bit, bool down);
    void applyMask();

    SDL_Gamepad *m_pad = nullptr;
    quint32 m_instanceId = 0;
    QString m_name;
    QTimer m_timer;

    // D-pad and stick each keep their own view of bits 0..3; the core
    // sees the OR so releasing one source never drops the other's hold.
    quint8 m_dpadMask = 0;
    quint8 m_stickMask = 0;
    quint8 m_buttonMask = 0; // bits 4..7 (a/b/select/start)
    quint8 m_sentMask = 0;
    bool m_shoulderTurbo = false;
    bool m_triggerTurbo = false;
    bool m_sentTurbo = false;
    bool m_sdlReady = false;
};
