// Emulator controller: owns the Rust core, runs it on a worker thread,
// pumps audio, persists battery saves / RTC, scans the ROM library.
#pragma once

#include <QAudioSink>
#include <QElapsedTimer>
#include <QImage>
#include <QMutex>
#include <QObject>
#include <QSettings>
#include <QTimer>
#include <QVariantList>
#include <atomic>
#include <thread>

struct GbHandle;

class Emulator : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool romLoaded READ romLoaded NOTIFY romChanged)
    Q_PROPERTY(bool paused READ paused NOTIFY pausedChanged)
    Q_PROPERTY(QString title READ title NOTIFY romChanged)
    Q_PROPERTY(QString romPath READ romPath NOTIFY romChanged)
    Q_PROPERTY(bool isCgb READ isCgb NOTIFY romChanged)
    Q_PROPERTY(double fps READ fps NOTIFY fpsChanged)
    Q_PROPERTY(bool turbo READ turbo WRITE setTurbo NOTIFY turboChanged)
    Q_PROPERTY(double volume READ volume WRITE setVolume NOTIFY volumeChanged)
    Q_PROPERTY(bool muted READ muted WRITE setMuted NOTIFY volumeChanged)
    Q_PROPERTY(int paletteMode READ paletteMode WRITE setPaletteMode NOTIFY paletteModeChanged)
    Q_PROPERTY(QString paletteName READ paletteName NOTIFY paletteModeChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY errorChanged)
    Q_PROPERTY(int stateSlot READ stateSlot WRITE setStateSlot NOTIFY optionsChanged)
    Q_PROPERTY(int turboSpeed READ turboSpeed WRITE setTurboSpeed NOTIFY optionsChanged)
    Q_PROPERTY(bool pauseOnFocusLoss READ pauseOnFocusLoss WRITE setPauseOnFocusLoss NOTIFY optionsChanged)
    Q_PROPERTY(bool integerScaling READ integerScaling WRITE setIntegerScaling NOTIFY optionsChanged)
    Q_PROPERTY(bool colorCorrection READ colorCorrection WRITE setColorCorrection NOTIFY optionsChanged)
    Q_PROPERTY(bool autoLoadLast READ autoLoadLast WRITE setAutoLoadLast NOTIFY optionsChanged)
    Q_PROPERTY(bool showFps READ showFps WRITE setShowFps NOTIFY optionsChanged)

public:
    explicit Emulator(class Theme *theme, QObject *parent = nullptr);
    ~Emulator() override;

    bool romLoaded() const { return m_gb != nullptr; }
    bool paused() const { return m_paused; }
    QString title() const { return m_title; }
    QString romPath() const { return m_romPath; }
    bool isCgb() const { return m_isCgb; }
    double fps() const { return m_fps; }
    bool turbo() const { return m_turbo; }
    void setTurbo(bool t);
    double volume() const { return m_volume; }
    void setVolume(double v);
    bool muted() const { return m_muted; }
    void setMuted(bool m);
    int paletteMode() const { return m_paletteMode; }
    void setPaletteMode(int mode);
    QString paletteName() const;
    QString lastError() const { return m_lastError; }

    /// Latest frame (copy), for the screen item.
    QImage currentFrame();

    // QoL options (persisted)
    int stateSlot() const { return m_stateSlot; }
    void setStateSlot(int s);
    int turboSpeed() const { return m_turboSpeed; }
    void setTurboSpeed(int s);
    bool pauseOnFocusLoss() const { return m_pauseOnFocusLoss; }
    void setPauseOnFocusLoss(bool v) { setOption("pauseOnFocusLoss", m_pauseOnFocusLoss, v); }
    bool integerScaling() const { return m_integerScaling; }
    void setIntegerScaling(bool v) { setOption("integerScaling", m_integerScaling, v); }
    bool colorCorrection() const { return m_colorCorrection; }
    void setColorCorrection(bool v);
    bool autoLoadLast() const { return m_autoLoadLast; }
    void setAutoLoadLast(bool v) { setOption("autoLoadLast", m_autoLoadLast, v); }
    bool showFps() const { return m_showFps; }
    void setShowFps(bool v) { setOption("showFps", m_showFps, v); }

    Q_INVOKABLE bool loadRom(const QString &path);
    Q_INVOKABLE void togglePause();
    Q_INVOKABLE void reset();
    Q_INVOKABLE void setButton(int bit, bool down);
    Q_INVOKABLE void cyclePalette();
    Q_INVOKABLE void saveState();
    Q_INVOKABLE void loadState();
    Q_INVOKABLE void nextSlot();
    Q_INVOKABLE void screenshot();

    // ROM library
    Q_INVOKABLE QVariantList scanLibrary();
    Q_INVOKABLE QStringList recentRoms() const;
    Q_INVOKABLE QString libraryDir() const;
    Q_INVOKABLE void setLibraryDir(const QString &dir);

public slots:
    void applyThemePalette();

signals:
    void frameReady();
    void romChanged();
    void pausedChanged();
    void fpsChanged();
    void turboChanged();
    void volumeChanged();
    void paletteModeChanged();
    void errorChanged();
    void optionsChanged();
    void toast(const QString &message);

private:
    void startWorker();
    void stopWorker();
    void workerLoop();
    void pumpAudio();
    void saveBattery(bool force);
    void pushDmgPalette();

    void setOption(const char *key, bool &field, bool value) {
        if (field == value)
            return;
        field = value;
        m_settings.setValue(key, value);
        emit optionsChanged();
    }
    QString statePath(int slot) const;

    class Theme *m_theme;
    GbHandle *m_gb = nullptr;
    QMutex m_coreMutex;

    std::thread m_worker;
    std::atomic<bool> m_running{false};
    std::atomic<bool> m_paused{false};
    std::atomic<uint8_t> m_buttons{0};
    std::atomic<bool> m_turbo{false};

    QImage m_frame;
    QMutex m_frameMutex;

    // Audio
    QAudioSink *m_sink = nullptr;
    QIODevice *m_sinkDev = nullptr;
    QByteArray m_ring;
    QMutex m_ringMutex;
    QTimer m_audioTimer;
    std::atomic<int> m_ringBytes{0};
    double m_volume = 0.8;
    bool m_muted = false;

    QTimer m_saveTimer;
    QSettings m_settings;

    QString m_romPath;
    QString m_title;
    QString m_lastError;
    bool m_isCgb = false;
    int m_paletteMode = 0;

    int m_stateSlot = 1;
    std::atomic<int> m_turboSpeed{4}; // read by the worker thread
    bool m_pauseOnFocusLoss = true;
    bool m_integerScaling = true;
    bool m_colorCorrection = true;
    bool m_autoLoadLast = false;
    bool m_showFps = true;

    std::atomic<int> m_frameCount{0};
    double m_fps = 0;
    QTimer m_fpsTimer;
};
