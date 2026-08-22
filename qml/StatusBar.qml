import QtQuick

Rectangle {
    id: bar
    height: 30
    color: theme.darkBackground

    property bool libraryOpen: false

    Rectangle {
        anchors.top: parent.top
        width: parent.width
        height: 1
        color: theme.lighterBackground
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8

        Text {
            text: emu.romLoaded ? emu.title.toLowerCase() : "no game"
            font.family: monoFont
            font.pixelSize: 12
            color: emu.romLoaded ? theme.foreground : theme.mutedColor
        }
        Text {
            visible: emu.romLoaded
            text: emu.isCgb ? "gbc" : "gb"
            font.family: monoFont
            font.pixelSize: 12
            color: emu.isCgb ? theme.cyan : theme.green
        }
        Text {
            visible: emu.paused
            text: "⏸ paused"
            font.family: monoFont
            font.pixelSize: 12
            color: theme.yellow
        }
        Text {
            visible: emu.turbo
            text: "»" + emu.turboSpeed + "×"
            font.family: monoFont
            font.pixelSize: 12
            color: theme.yellow
        }
    }

    // Context-sensitive shortcut hints, always visible in the bar.
    Text {
        anchors.centerIn: parent
        visible: bar.width >= (bar.libraryOpen ? 900 : emu.romLoaded ? 880 : 700)
        text: bar.libraryOpen
              ? "enter play · ctrl+o open file · ctrl+d set folder · f1 help · f2 settings · esc close"
              : emu.romLoaded
                ? input.keyName("pause") + " pause · esc library · f1 help · f2 settings"
                : "esc library · f1 help · f2 settings"
        font.family: monoFont
        font.pixelSize: 11
        color: theme.mutedColor
    }

    Row {
        visible: !bar.libraryOpen
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 14

        Text {
            visible: emu.romLoaded
            text: "slot " + emu.stateSlot
            font.family: monoFont
            font.pixelSize: 12
            color: theme.mutedColor
        }
        Text {
            visible: emu.romLoaded && !emu.isCgb
            text: "◧ " + emu.paletteName
            font.family: monoFont
            font.pixelSize: 12
            color: theme.mutedColor
        }
        Text {
            text: emu.muted ? "vol ✗" : "vol " + Math.round(emu.volume * 100) + "%"
            font.family: monoFont
            font.pixelSize: 12
            color: emu.muted ? theme.red : theme.mutedColor
        }
        Text {
            visible: emu.romLoaded && !emu.paused && emu.showFps
            text: Math.round(emu.fps) + " fps"
            font.family: monoFont
            font.pixelSize: 12
            color: Math.round(emu.fps) >= 59 || emu.fps === 0
                   ? theme.mutedColor : theme.yellow
        }
    }
}
