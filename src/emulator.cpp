#include "emulator.h"
#include "gbcore.h"
#include "theme.h"

#include <QAudioFormat>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QDateTime>
#include <QMediaDevices>
#include <QSet>
#include <QStandardPaths>
#include <QThread>
#include <QUrl>

namespace {
constexpr int kWidth = 160;
constexpr int kHeight = 144;
constexpr int kSampleRate = 48000;
constexpr double kFrameSeconds = 70224.0 / 4194304.0;

// Ring buffer: ~80 ms target fill, 250 ms hard cap (bytes, stereo f32).
constexpr int kRingTarget = int(kSampleRate * 0.08) * 2 * 4;
constexpr int kRingMax = int(kSampleRate * 0.25) * 2 * 4;

QString sidecar(const QString &romPath, const QString &ext) {
    QFileInfo fi(romPath);
    return fi.dir().filePath(fi.completeBaseName() + ext);
}
} // namespace

Emulator::Emulator(Theme *theme, QObject *parent)
    : QObject(parent), m_theme(theme),
      m_settings("omaboy", "omaboy") {
    m_frame = QImage(kWidth, kHeight, QImage::Format_RGB32);
    m_frame.fill(m_theme->background());

    m_volume = m_settings.value("volume", 0.8).toDouble();
    m_muted = m_settings.value("muted", false).toBool();
    // Default to the classic DMG palette so the omarchy theme doesn't
    // recolor the game itself; "omarchy" tint stays available via `p`.
    m_paletteMode = m_settings.value("paletteMode", 1).toInt();

    m_stateSlot = qBound(1, m_settings.value("stateSlot", 1).toInt(), 3);
    m_turboSpeed = qBound(2, m_settings.value("turboSpeed", 4).toInt(), 8);
    m_pauseOnFocusLoss = m_settings.value("pauseOnFocusLoss", true).toBool();
    m_integerScaling = m_settings.value("integerScaling", true).toBool();
    m_colorCorrection = m_settings.value("colorCorrection", true).toBool();
    m_autoLoadLast = m_settings.value("autoLoadLast", false).toBool();
    m_showFps = m_settings.value("showFps", true).toBool();

    connect(m_theme, &Theme::changed, this, &Emulator::applyThemePalette);

    // Audio sink on the GUI thread; a timer pumps samples from the ring.
    QAudioFormat fmt;
    fmt.setSampleRate(kSampleRate);
    fmt.setChannelCount(2);
    fmt.setSampleFormat(QAudioFormat::Float);
    const QAudioDevice dev = QMediaDevices::defaultAudioOutput();
    if (!dev.isNull() && dev.isFormatSupported(fmt)) {
        m_sink = new QAudioSink(dev, fmt, this);
        m_sink->setBufferSize(kRingTarget * 2);
        m_sinkDev = m_sink->start();
    }
    m_audioTimer.setInterval(8);
    m_audioTimer.setTimerType(Qt::PreciseTimer);
    connect(&m_audioTimer, &QTimer::timeout, this, &Emulator::pumpAudio);

    m_saveTimer.setInterval(15000);
    connect(&m_saveTimer, &QTimer::timeout, this, [this] { saveBattery(false); });

    m_fpsTimer.setInterval(1000);
    connect(&m_fpsTimer, &QTimer::timeout, this, [this] {
        m_fps = m_frameCount.exchange(0);
        emit fpsChanged();
    });
}

Emulator::~Emulator() {
    stopWorker();
    saveBattery(true);
    QMutexLocker lock(&m_coreMutex);
    if (m_gb) {
        gb_destroy(m_gb);
        m_gb = nullptr;
    }
}

QImage Emulator::currentFrame() {
    QMutexLocker lock(&m_frameMutex);
    return m_frame;
}

bool Emulator::loadRom(const QString &pathOrUrl) {
    QString path = pathOrUrl;
    if (path.startsWith("file://"))
        path = QUrl(pathOrUrl).toLocalFile();

    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        m_lastError = "cannot read " + path;
        emit errorChanged();
        return false;
    }
    const QByteArray data = f.readAll();

    GbHandle *gb = gb_create(reinterpret_cast<const uint8_t *>(data.constData()),
                             size_t(data.size()));
    if (!gb) {
        m_lastError = "not a valid Game Boy ROM: " + QFileInfo(path).fileName();
        emit errorChanged();
        return false;
    }

    stopWorker();
    saveBattery(true);
    {
        QMutexLocker lock(&m_coreMutex);
        if (m_gb)
            gb_destroy(m_gb);
        m_gb = gb;
        m_romPath = path;
        m_isCgb = gb_is_cgb(gb);

        char title[32] = {};
        gb_title(gb, title, sizeof title);
        m_title = QString::fromLatin1(title);
        if (m_title.isEmpty())
            m_title = QFileInfo(path).completeBaseName();

        gb_set_color_correction(gb, m_colorCorrection);

        // Battery save + RTC
        QFile sav(sidecar(path, ".sav"));
        if (sav.open(QIODevice::ReadOnly)) {
            const QByteArray s = sav.readAll();
            gb_battery_load(gb, reinterpret_cast<const uint8_t *>(s.constData()),
                            size_t(s.size()));
        }
        QFile rtc(sidecar(path, ".rtc"));
        if (rtc.open(QIODevice::ReadOnly)) {
            const QByteArray r = rtc.readAll();
            gb_rtc_load(gb, reinterpret_cast<const uint8_t *>(r.constData()),
                        size_t(r.size()));
        }
    }
    pushDmgPalette();

    // Remember in recents.
    QStringList recents = m_settings.value("recentRoms").toStringList();
    recents.removeAll(path);
    recents.prepend(path);
    while (recents.size() > 10)
        recents.removeLast();
    m_settings.setValue("recentRoms", recents);

    m_lastError.clear();
    m_paused = false;
    emit errorChanged();
    emit romChanged();
    emit pausedChanged();
    startWorker();
    return true;
}

void Emulator::togglePause() {
    if (!m_gb)
        return;
    m_paused = !m_paused;
    if (m_paused)
        saveBattery(false);
    emit pausedChanged();
}

void Emulator::reset() {
    QMutexLocker lock(&m_coreMutex);
    if (m_gb)
        gb_reset(m_gb);
}

void Emulator::setButton(int bit, bool down) {
    uint8_t b = m_buttons.load();
    if (down)
        b |= uint8_t(1u << bit);
    else
        b &= uint8_t(~(1u << bit));
    m_buttons = b;
}

void Emulator::setTurbo(bool t) {
    if (m_turbo == t)
        return;
    m_turbo = t;
    emit turboChanged();
}

void Emulator::setVolume(double v) {
    m_volume = qBound(0.0, v, 1.0);
    m_settings.setValue("volume", m_volume);
    emit volumeChanged();
}

void Emulator::setMuted(bool m) {
    m_muted = m;
    m_settings.setValue("muted", m);
    emit volumeChanged();
}

void Emulator::setPaletteMode(int mode) {
    m_paletteMode = ((mode % 3) + 3) % 3;
    m_settings.setValue("paletteMode", m_paletteMode);
    pushDmgPalette();
    emit paletteModeChanged();
}

void Emulator::cyclePalette() { setPaletteMode(m_paletteMode + 1); }

void Emulator::setStateSlot(int s) {
    s = qBound(1, s, 3);
    if (s == m_stateSlot)
        return;
    m_stateSlot = s;
    m_settings.setValue("stateSlot", s);
    emit optionsChanged();
}

void Emulator::setTurboSpeed(int s) {
    s = qBound(2, s, 8);
    if (s == m_turboSpeed)
        return;
    m_turboSpeed = s;
    m_settings.setValue("turboSpeed", s);
    emit optionsChanged();
}

void Emulator::setColorCorrection(bool v) {
    if (m_colorCorrection == v)
        return;
    m_colorCorrection = v;
    m_settings.setValue("colorCorrection", v);
    QMutexLocker lock(&m_coreMutex);
    if (m_gb)
        gb_set_color_correction(m_gb, v);
    emit optionsChanged();
}

QString Emulator::statePath(int slot) const {
    return sidecar(m_romPath, QString(".st%1").arg(slot));
}

void Emulator::saveState() {
    QByteArray data;
    {
        QMutexLocker lock(&m_coreMutex);
        if (!m_gb)
            return;
        const size_t size = gb_state_save(m_gb, nullptr, 0);
        data.resize(qsizetype(size));
        if (gb_state_save(m_gb, reinterpret_cast<uint8_t *>(data.data()), size) != size)
            return;
    }
    QFile f(statePath(m_stateSlot));
    if (f.open(QIODevice::WriteOnly) && f.write(data) == data.size())
        emit toast(QString("state saved · slot %1").arg(m_stateSlot));
    else
        emit toast("could not write save state");
}

void Emulator::loadState() {
    if (!m_gb)
        return;
    QFile f(statePath(m_stateSlot));
    if (!f.open(QIODevice::ReadOnly)) {
        emit toast(QString("no state in slot %1").arg(m_stateSlot));
        return;
    }
    const QByteArray data = f.readAll();
    bool ok;
    {
        QMutexLocker lock(&m_coreMutex);
        ok = gb_state_load(m_gb, reinterpret_cast<const uint8_t *>(data.constData()),
                           size_t(data.size()));
        if (ok) {
            QMutexLocker fl(&m_frameMutex);
            memcpy(m_frame.bits(), gb_framebuffer(m_gb), size_t(kWidth) * kHeight * 4);
        }
    }
    if (ok) {
        emit frameReady();
        emit toast(QString("state loaded · slot %1").arg(m_stateSlot));
    } else {
        emit toast(QString("slot %1: not a state for this game").arg(m_stateSlot));
    }
}

void Emulator::nextSlot() {
    setStateSlot(m_stateSlot % 3 + 1);
    const bool exists = m_romPath.isEmpty() ? false : QFile::exists(statePath(m_stateSlot));
    emit toast(QString("slot %1%2").arg(m_stateSlot).arg(exists ? "" : " · empty"));
}

void Emulator::screenshot() {
    if (!m_gb)
        return;
    const QImage shot =
        currentFrame().scaled(kWidth * 4, kHeight * 4, Qt::IgnoreAspectRatio,
                              Qt::FastTransformation);
    const QString dir =
        QStandardPaths::writableLocation(QStandardPaths::PicturesLocation) + "/omaboy";
    QDir().mkpath(dir);
    const QString name = QString("%1-%2.png")
                             .arg(m_title.toLower().replace(' ', '-'))
                             .arg(QDateTime::currentDateTime().toString("yyyyMMdd-hhmmss"));
    if (shot.save(dir + "/" + name))
        emit toast("screenshot · " + name);
    else
        emit toast("screenshot failed");
}

QString Emulator::paletteName() const {
    switch (m_paletteMode) {
    case 1: return "classic";
    case 2: return "mono";
    default: return "omarchy";
    }
}

void Emulator::applyThemePalette() {
    pushDmgPalette();
    if (!m_gb) {
        QMutexLocker lock(&m_frameMutex);
        m_frame.fill(m_theme->background());
        emit frameReady();
    }
}

void Emulator::pushDmgPalette() {
    uint32_t colors[4];
    switch (m_paletteMode) {
    case 1: { // classic DMG green
        const uint32_t classic[4] = {0xFFE0F8D0, 0xFF88C070, 0xFF346856, 0xFF081820};
        std::copy(classic, classic + 4, colors);
        break;
    }
    case 2: { // grayscale
        const uint32_t mono[4] = {0xFFF3F3F3, 0xFFAAAAAA, 0xFF555555, 0xFF101010};
        std::copy(mono, mono + 4, colors);
        break;
    }
    default: { // derived from the active omarchy theme
        const QList<QColor> p = m_theme->dmgPalette();
        for (int i = 0; i < 4; ++i)
            colors[i] = 0xFF000000u | uint32_t(p[i].rgb() & 0xFFFFFF);
        break;
    }
    }
    QMutexLocker lock(&m_coreMutex);
    if (m_gb)
        gb_set_dmg_palette(m_gb, colors);
}

// ---- worker ----

void Emulator::startWorker() {
    if (m_running)
        return;
    m_running = true;
    m_worker = std::thread([this] { workerLoop(); });
    m_audioTimer.start();
    m_saveTimer.start();
    m_fpsTimer.start();
}

void Emulator::stopWorker() {
    if (!m_running)
        return;
    m_running = false;
    if (m_worker.joinable())
        m_worker.join();
    m_audioTimer.stop();
    m_saveTimer.stop();
    m_fpsTimer.stop();
    m_fps = 0;
    emit fpsChanged();
}

void Emulator::workerLoop() {
    QElapsedTimer clock;
    clock.start();
    double nextFrame = 0;

    while (m_running) {
        if (m_paused) {
            QThread::msleep(8);
            clock.restart();
            nextFrame = 0;
            continue;
        }

        const int framesThisTick = m_turbo ? m_turboSpeed.load() : 1;
        {
            QMutexLocker lock(&m_coreMutex);
            if (!m_gb)
                break;
            gb_set_buttons(m_gb, m_buttons.load());
            for (int i = 0; i < framesThisTick; ++i)
                gb_run_frame(m_gb);

            // Move audio into the ring (drop when turbo or over-full).
            float buf[4096];
            size_t n;
            while ((n = gb_audio_read(m_gb, buf, 4096)) > 0) {
                if (m_turbo || m_muted)
                    continue;
                const float gain = float(m_volume) * float(m_volume);
                for (size_t i = 0; i < n; ++i)
                    buf[i] *= gain;
                QMutexLocker rl(&m_ringMutex);
                if (m_ring.size() < kRingMax)
                    m_ring.append(reinterpret_cast<const char *>(buf),
                                  qsizetype(n * sizeof(float)));
                m_ringBytes = int(m_ring.size());
            }

            {
                QMutexLocker fl(&m_frameMutex);
                memcpy(m_frame.bits(), gb_framebuffer(m_gb),
                       size_t(kWidth) * kHeight * 4);
            }
        }
        m_frameCount.fetch_add(1);
        emit frameReady();

        // Pace to 59.73 Hz, nudged by ring fill so emu and audio clocks
        // stay locked (drops/underruns stay inaudible).
        double period = kFrameSeconds * 1e9;
        if (m_turbo)
            period = 1e9 / 60.0; // turboSpeed frames per tick = turboSpeed × speed
        else if (m_sinkDev && !m_muted) {
            const int fill = m_ringBytes.load();
            const double err = double(fill - kRingTarget) / kRingTarget;
            period *= 1.0 + qBound(-0.03, err * 0.05, 0.03);
        }
        nextFrame += period;
        const double now = double(clock.nsecsElapsed());
        double wait = nextFrame - now;
        if (wait < -50e6) { // fell far behind; resync
            nextFrame = now;
            wait = 0;
        }
        if (wait > 0)
            QThread::usleep(unsigned(wait / 1000.0));
    }
}

void Emulator::pumpAudio() {
    if (!m_sinkDev)
        return;
    const qint64 free = m_sink->bytesFree();
    if (free <= 0)
        return;
    QMutexLocker rl(&m_ringMutex);
    const qint64 n = qMin<qint64>(free, m_ring.size());
    if (n > 0) {
        m_sinkDev->write(m_ring.constData(), n);
        m_ring.remove(0, int(n));
        m_ringBytes = int(m_ring.size());
    }
}

void Emulator::saveBattery(bool force) {
    QMutexLocker lock(&m_coreMutex);
    if (!m_gb || m_romPath.isEmpty() || !gb_has_battery(m_gb))
        return;
    const bool dirty = gb_battery_take_dirty(m_gb);
    if (!dirty && !force)
        return;

    const size_t size = gb_battery_size(m_gb);
    if (size > 0) {
        QFile f(sidecar(m_romPath, ".sav"));
        if (f.open(QIODevice::WriteOnly))
            f.write(reinterpret_cast<const char *>(gb_battery_data(m_gb)),
                    qint64(size));
    }
    if (gb_has_rtc(m_gb)) {
        uint8_t rtc[44];
        if (gb_rtc_save(m_gb, rtc, sizeof rtc) == sizeof rtc) {
            QFile f(sidecar(m_romPath, ".rtc"));
            if (f.open(QIODevice::WriteOnly))
                f.write(reinterpret_cast<const char *>(rtc), sizeof rtc);
        }
    }
}

// ---- library ----

QString Emulator::libraryDir() const {
    const QString def = QDir::homePath() + "/Games";
    return m_settings.value("libraryDir", def).toString();
}

void Emulator::setLibraryDir(const QString &dirOrUrl) {
    QString dir = dirOrUrl;
    if (dir.startsWith("file://"))
        dir = QUrl(dirOrUrl).toLocalFile();
    m_settings.setValue("libraryDir", dir);
}

QVariantList Emulator::scanLibrary() {
    QVariantList out;
    QStringList dirs{libraryDir()};
    for (const QString &extra : {QDir::homePath() + "/ROMs", QDir::homePath() + "/roms"})
        if (!dirs.contains(extra))
            dirs << extra;

    QSet<QString> seen;
    for (const QString &dir : dirs) {
        if (!QDir(dir).exists())
            continue;
        QDirIterator it(dir, {"*.gb", "*.gbc", "*.zip"}, QDir::Files,
                        QDirIterator::Subdirectories);
        while (it.hasNext()) {
            const QString path = it.next();
            if (seen.contains(path))
                continue;
            seen.insert(path);
            QVariantMap m;
            m["name"] = QFileInfo(path).completeBaseName();
            m["path"] = path;
            m["ext"] = QFileInfo(path).suffix().toLower();
            out.append(m);
        }
    }
    std::sort(out.begin(), out.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap()["name"].toString().localeAwareCompare(
                   b.toMap()["name"].toString()) < 0;
    });
    return out;
}

QStringList Emulator::recentRoms() const {
    QStringList recents = m_settings.value("recentRoms").toStringList();
    recents.removeIf([](const QString &p) { return !QFile::exists(p); });
    return recents;
}
