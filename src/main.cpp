#include "emulator.h"
#include "inputmap.h"
#include "theme.h"

#include <QDir>
#include <QFile>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QStandardPaths>

// One-time migration from the pre-rebrand config ("omulator").
static void migrateOldSettings() {
    const QString cfg = QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation);
    const QString oldFile = cfg + "/omulator/omulator.conf";
    const QString newFile = cfg + "/omaboy/omaboy.conf";
    if (QFile::exists(oldFile) && !QFile::exists(newFile)) {
        QDir().mkpath(cfg + "/omaboy");
        QFile::copy(oldFile, newFile);
    }
}

int main(int argc, char *argv[]) {
    for (int i = 1; i < argc; ++i) {
        if (qstrcmp(argv[i], "--version") == 0 || qstrcmp(argv[i], "-v") == 0) {
            printf("omaboy %s\n", OMABOY_VERSION);
            return 0;
        }
    }

    QGuiApplication app(argc, argv);
    app.setApplicationName("omaboy");
    app.setOrganizationName("omaboy");
    app.setApplicationVersion(OMABOY_VERSION);
    app.setDesktopFileName("omaboy"); // Wayland app-id, for Hyprland rules
    migrateOldSettings();

    Theme theme;
    Emulator emulator(&theme);
    InputMap input;

    QFont mono("JetBrainsMono Nerd Font");
    if (!QFontDatabase::families().contains(mono.family()))
        mono = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    mono.setPixelSize(13);
    app.setFont(mono);

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("theme", &theme);
    engine.rootContext()->setContextProperty("emu", &emulator);
    engine.rootContext()->setContextProperty("input", &input);
    engine.rootContext()->setContextProperty("monoFont", mono.family());

    // omaboy <rom> loads straight into the game (before QML decides
    // whether to open the library).
    const QStringList args = app.arguments();
    if (args.size() > 1)
        emulator.loadRom(args.at(1));
    else if (emulator.autoLoadLast() && !emulator.recentRoms().isEmpty())
        emulator.loadRom(emulator.recentRoms().first());

    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("Omaboy", "Main");

    return app.exec();
}
