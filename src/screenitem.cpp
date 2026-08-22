#include "screenitem.h"
#include "emulator.h"

#include <QPainter>

ScreenItem::ScreenItem(QQuickItem *parent) : QQuickPaintedItem(parent) {
    setRenderTarget(QQuickPaintedItem::FramebufferObject);
}

void ScreenItem::setEmulator(Emulator *e) {
    if (m_emulator == e)
        return;
    if (m_emulator)
        disconnect(m_emulator, nullptr, this, nullptr);
    m_emulator = e;
    if (m_emulator)
        connect(m_emulator, &Emulator::frameReady, this,
                [this] { update(); }, Qt::QueuedConnection);
    emit emulatorChanged();
    update();
}

void ScreenItem::paint(QPainter *painter) {
    if (!m_emulator)
        return;
    const QImage frame = m_emulator->currentFrame();
    painter->setRenderHint(QPainter::SmoothPixmapTransform, false);
    painter->drawImage(QRectF(0, 0, width(), height()), frame);
}
