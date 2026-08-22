// Live Omarchy theme integration: parses the active theme's colors.toml and
// hot-reloads when the user switches themes (omarchy theme set ...).
#pragma once

#include <QColor>
#include <QFileSystemWatcher>
#include <QHash>
#include <QObject>
#include <QTimer>

class Theme : public QObject {
    Q_OBJECT
    Q_PROPERTY(QColor background READ background NOTIFY changed)
    Q_PROPERTY(QColor darkBackground READ darkBackground NOTIFY changed)
    Q_PROPERTY(QColor darkerBackground READ darkerBackground NOTIFY changed)
    Q_PROPERTY(QColor lighterBackground READ lighterBackground NOTIFY changed)
    Q_PROPERTY(QColor foreground READ foreground NOTIFY changed)
    Q_PROPERTY(QColor lightForeground READ lightForeground NOTIFY changed)
    Q_PROPERTY(QColor mutedColor READ mutedColor NOTIFY changed)
    Q_PROPERTY(QColor accent READ accent NOTIFY changed)
    Q_PROPERTY(QColor selection READ selection NOTIFY changed)
    Q_PROPERTY(QColor red READ red NOTIFY changed)
    Q_PROPERTY(QColor green READ green NOTIFY changed)
    Q_PROPERTY(QColor yellow READ yellow NOTIFY changed)
    Q_PROPERTY(QColor cyan READ cyan NOTIFY changed)
    Q_PROPERTY(bool dark READ dark NOTIFY changed)
    Q_PROPERTY(QString themeName READ themeName NOTIFY changed)

public:
    explicit Theme(QObject *parent = nullptr);

    QColor background() const { return get("background", "#24273a"); }
    QColor darkBackground() const { return get("dark_background", background().darker(115).name()); }
    QColor darkerBackground() const { return get("darker_background", background().darker(130).name()); }
    QColor lighterBackground() const { return get("lighter_background", background().lighter(115).name()); }
    QColor foreground() const { return get("foreground", "#cad3f5"); }
    QColor lightForeground() const { return get("light_foreground", foreground().name()); }
    QColor mutedColor() const { return get("muted", "#6e738d"); }
    QColor accent() const { return get("accent", "#8aadf4"); }
    QColor selection() const { return get("selection", lighterBackground().name()); }
    QColor red() const { return get("red", "#ed8796"); }
    QColor green() const { return get("green", "#a6da95"); }
    QColor yellow() const { return get("yellow", "#eed49f"); }
    QColor cyan() const { return get("cyan", "#8bd5ca"); }
    bool dark() const { return m_values.value("mode", "dark") != "light"; }
    QString themeName() const { return m_themeName; }

    /// Four DMG shade colors derived from the theme (index 0 = lightest shade).
    QList<QColor> dmgPalette() const;

signals:
    void changed();

private:
    void reload();
    void rewatch();
    QColor get(const QString &key, const QString &fallback) const;

    QString m_stateDir;
    QString m_themeName;
    QHash<QString, QString> m_values;
    QFileSystemWatcher m_watcher;
    QTimer m_debounce;
};
