// Rebindable keyboard controls, persisted in QSettings.
#pragma once

#include <QObject>
#include <QSettings>
#include <QVariantList>

class InputMap : public QObject {
    Q_OBJECT

public:
    explicit InputMap(QObject *parent = nullptr);

    /// Action id for a Qt key, or "" if unbound. Keypad-enter folds into return.
    Q_INVOKABLE QString actionForKey(int key) const;

    /// Game Boy button bit for an action (0=Right..7=Start), or -1.
    Q_INVOKABLE int buttonBit(const QString &action) const;

    Q_INVOKABLE QString keyName(const QString &action) const;
    Q_INVOKABLE void rebind(const QString &action, int key);
    Q_INVOKABLE void resetDefaults();

    /// Rows for the settings UI: {id, label, keyName}.
    Q_INVOKABLE QVariantList model() const;

signals:
    void changed();

private:
    struct Action {
        const char *id;
        const char *label;
        int defaultKey;
        int bit; // GB button bit or -1
    };
    static const QList<Action> &actions();
    void load();
    void save();

    QHash<QString, int> m_keys;
    QSettings m_settings;
};
