#include "theme.h"

#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QStandardPaths>

Theme::Theme(QObject *parent) : QObject(parent) {
    const QString state =
        qEnvironmentVariable("XDG_STATE_HOME", QDir::homePath() + "/.local/state");
    m_stateDir = state + "/omarchy/current";

    m_debounce.setSingleShot(true);
    m_debounce.setInterval(150);
    connect(&m_debounce, &QTimer::timeout, this, [this] {
        reload();
        rewatch();
        emit changed();
    });
    auto poke = [this](const QString &) { m_debounce.start(); };
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, poke);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, poke);

    reload();
    rewatch();
}

void Theme::reload() {
    m_values.clear();
    m_themeName.clear();

    QFile nameFile(m_stateDir + "/theme.name");
    if (nameFile.open(QIODevice::ReadOnly))
        m_themeName = QString::fromUtf8(nameFile.readAll()).trimmed();

    QFile f(m_stateDir + "/theme/colors.toml");
    if (!f.open(QIODevice::ReadOnly))
        return;

    // colors.toml is flat `key = "value"` pairs; a tiny parser is enough.
    static const QRegularExpression line(
        R"(^\s*([A-Za-z0-9_]+)\s*=\s*"([^"]*)\")");
    while (!f.atEnd()) {
        const auto m = line.match(QString::fromUtf8(f.readLine()));
        if (m.hasMatch())
            m_values.insert(m.captured(1), m.captured(2));
    }
}

void Theme::rewatch() {
    if (!m_watcher.files().isEmpty())
        m_watcher.removePaths(m_watcher.files());
    if (!m_watcher.directories().isEmpty())
        m_watcher.removePaths(m_watcher.directories());
    // theme.name is rewritten on every switch; the dir catches symlink swaps.
    for (const QString &p :
         {m_stateDir, m_stateDir + "/theme.name", m_stateDir + "/theme/colors.toml"}) {
        if (QFile::exists(p))
            m_watcher.addPath(p);
    }
}

QColor Theme::get(const QString &key, const QString &fallback) const {
    const QString v = m_values.value(key);
    QColor c(v);
    return c.isValid() ? c : QColor(fallback);
}

QList<QColor> Theme::dmgPalette() const {
    // Shade 0 (lightest on hardware) = theme background, shade 3 = foreground:
    // games take on the look of the terminal colorscheme.
    const QColor bg = background();
    const QColor fg = foreground();
    auto mix = [&](qreal t) {
        return QColor::fromRgbF(bg.redF() + (fg.redF() - bg.redF()) * t,
                                bg.greenF() + (fg.greenF() - bg.greenF()) * t,
                                bg.blueF() + (fg.blueF() - bg.blueF()) * t);
    };
    return {bg, mix(0.35), mix(0.68), fg};
}
