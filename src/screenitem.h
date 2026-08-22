// QML item that paints the emulator framebuffer with crisp integer scaling.
#pragma once

#include <QQuickPaintedItem>

class Emulator;

class ScreenItem : public QQuickPaintedItem {
    Q_OBJECT
    Q_PROPERTY(Emulator *emulator READ emulator WRITE setEmulator NOTIFY emulatorChanged)
    QML_ELEMENT

public:
    explicit ScreenItem(QQuickItem *parent = nullptr);

    Emulator *emulator() const { return m_emulator; }
    void setEmulator(Emulator *e);

    void paint(QPainter *painter) override;

signals:
    void emulatorChanged();

private:
    Emulator *m_emulator = nullptr;
};
