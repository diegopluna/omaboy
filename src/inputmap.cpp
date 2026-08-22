#include "inputmap.h"

#include <QKeySequence>

const QList<InputMap::Action> &InputMap::actions() {
    static const QList<Action> list = {
        {"up", "d-pad up", Qt::Key_Up, 2},
        {"down", "d-pad down", Qt::Key_Down, 3},
        {"left", "d-pad left", Qt::Key_Left, 1},
        {"right", "d-pad right", Qt::Key_Right, 0},
        {"a", "button a", Qt::Key_X, 4},
        {"b", "button b", Qt::Key_Z, 5},
        {"start", "start", Qt::Key_Return, 7},
        {"select", "select", Qt::Key_Shift, 6},
        {"turbo", "turbo (hold)", Qt::Key_Space, -1},
        {"pause", "pause", Qt::Key_Tab, -1},
        {"save_state", "save state", Qt::Key_F5, -1},
        {"load_state", "load state", Qt::Key_F8, -1},
        {"next_slot", "next state slot", Qt::Key_F6, -1},
        {"screenshot", "screenshot", Qt::Key_F12, -1},
        {"reset", "reset game", Qt::Key_R, -1},
        {"palette", "cycle palette", Qt::Key_P, -1},
        {"mute", "mute", Qt::Key_M, -1},
        {"fullscreen", "fullscreen", Qt::Key_F, -1},
    };
    return list;
}

InputMap::InputMap(QObject *parent)
    : QObject(parent), m_settings("omaboy", "omaboy") {
    load();
}

void InputMap::load() {
    m_keys.clear();
    for (const Action &a : actions())
        m_keys[a.id] = m_settings.value("bindings/" + QString(a.id), a.defaultKey).toInt();
}

void InputMap::save() {
    for (auto it = m_keys.cbegin(); it != m_keys.cend(); ++it)
        m_settings.setValue("bindings/" + it.key(), it.value());
}

static int normalizeKey(int key) {
    return key == Qt::Key_Enter ? Qt::Key_Return : key;
}

QString InputMap::actionForKey(int key) const {
    const int k = normalizeKey(key);
    if (k == 0)
        return {};
    for (auto it = m_keys.cbegin(); it != m_keys.cend(); ++it)
        if (it.value() == k)
            return it.key();
    return {};
}

int InputMap::buttonBit(const QString &action) const {
    for (const Action &a : actions())
        if (action == a.id)
            return a.bit;
    return -1;
}

QString InputMap::keyName(const QString &action) const {
    const int key = m_keys.value(action, 0);
    switch (key) {
    case 0: return "—";
    case Qt::Key_Shift: return "shift";
    case Qt::Key_Control: return "ctrl";
    case Qt::Key_Alt: return "alt";
    case Qt::Key_Meta: return "super";
    case Qt::Key_Return: return "enter";
    case Qt::Key_Space: return "space";
    default: return QKeySequence(key).toString(QKeySequence::PortableText).toLower();
    }
}

void InputMap::rebind(const QString &action, int key) {
    const int k = normalizeKey(key);
    if (!m_keys.contains(action) || k == 0)
        return;
    // Reserved application keys stay fixed.
    if (k == Qt::Key_Escape || k == Qt::Key_F1 || k == Qt::Key_F2 || k == Qt::Key_F11)
        return;
    // A key can drive only one action: the previous owner becomes unbound.
    for (auto it = m_keys.begin(); it != m_keys.end(); ++it)
        if (it.value() == k && it.key() != action)
            it.value() = 0;
    m_keys[action] = k;
    save();
    emit changed();
}

void InputMap::resetDefaults() {
    for (const Action &a : actions())
        m_keys[a.id] = a.defaultKey;
    save();
    emit changed();
}

QVariantList InputMap::model() const {
    QVariantList out;
    for (const Action &a : actions()) {
        QVariantMap m;
        m["id"] = a.id;
        m["label"] = a.label;
        m["keyName"] = keyName(a.id);
        out.append(m);
    }
    return out;
}
