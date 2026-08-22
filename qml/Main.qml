import QtQuick
import QtQuick.Window
import Omaboy

Window {
    id: root
    width: 964
    height: 900
    minimumWidth: 480
    minimumHeight: 500
    visible: true
    title: emu.romLoaded ? emu.title.toLowerCase() + " — omaboy" : "omaboy"
    color: theme.background

    Item {
        id: game
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top

        // Crisp integer scaling whenever there's room for it (toggle in settings).
        readonly property int availW: width - 48
        readonly property int availH: height - 48
        readonly property real fitScale: Math.min(availW / 160, availH / 144)
        readonly property real scaleFactor:
            emu.integerScaling && fitScale >= 1 ? Math.floor(fitScale) : fitScale

        Rectangle {
            id: bezel
            anchors.centerIn: parent
            width: 160 * game.scaleFactor + 4
            height: 144 * game.scaleFactor + 4
            color: "transparent"
            border.width: 2
            border.color: emu.romLoaded && !emu.paused ? theme.accent : theme.mutedColor
            visible: emu.romLoaded

            Behavior on border.color { ColorAnimation { duration: 150 } }

            ScreenItem {
                anchors.fill: parent
                anchors.margins: 2
                emulator: emu
            }

            Rectangle {
                anchors.fill: parent
                color: theme.background
                opacity: emu.paused ? 0.55 : 0
                Behavior on opacity { NumberAnimation { duration: 120 } }
            }
            Text {
                anchors.centerIn: parent
                visible: emu.paused
                text: "paused"
                font.family: monoFont
                font.pixelSize: 22
                color: theme.foreground
            }
        }

        // Idle splash
        Column {
            anchors.centerIn: parent
            spacing: 18
            visible: !emu.romLoaded

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "▄▄ omaboy ▄▄"
                font.family: monoFont
                font.pixelSize: 30
                font.bold: true
                color: theme.accent
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "game boy · game boy color"
                font.family: monoFont
                font.pixelSize: 13
                color: theme.mutedColor
            }
            Item { width: 1; height: 8 }
            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 6
                Repeater {
                    model: [
                        ["esc", "game library"],
                        ["f1", "keybindings"],
                        ["f2", "settings"]
                    ]
                    delegate: Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 10
                        Text {
                            text: modelData[0]
                            font.family: monoFont
                            font.pixelSize: 13
                            color: theme.accent
                        }
                        Text {
                            text: modelData[1]
                            font.family: monoFont
                            font.pixelSize: 13
                            color: theme.lightForeground
                        }
                    }
                }
            }
        }
    }

    StatusBar {
        id: statusBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        libraryOpen: browser.visible
    }

    // Overlays stop above the status bar so its hints stay readable.
    RomBrowser {
        id: browser
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        onClosed: keys.forceActiveFocus()
        onRequestHelp: help.toggle()
        onRequestSettings: settings.toggle()
    }

    HelpOverlay {
        id: help
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        onClosed: browser.visible ? browser.refocus() : keys.forceActiveFocus()
    }

    SettingsOverlay {
        id: settings
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        onClosed: browser.visible ? browser.refocus() : keys.forceActiveFocus()
    }

    Toast { id: toast }

    // Controller input lands in the same funnel as the keyboard. The
    // browser gets first refusal on navigation so a pad can drive the
    // library too; otherwise buttons go to the game.
    Connections {
        target: pad
        function onButtonChanged(bit, down) {
            if (settings.visible || help.visible)
                return
            if (browser.visible) {
                if (down) browser.padNavigate(bit)
                return
            }
            emu.setButton(bit, down)
        }
        function onActionEvent(action, down) {
            if (settings.visible || help.visible)
                return
            if (browser.visible) {
                if (down) browser.padAction(action)
                return
            }
            const bit = input.buttonBit(action)
            if (bit >= 0) { emu.setButton(bit, down); return }
            if (action === "turbo") { emu.turbo = down; return }
            if (!down) return
            switch (action) {
            case "pause": emu.togglePause(); break
            case "save_state": emu.saveState(); break
            case "load_state": emu.loadState(); break
            case "next_slot": emu.nextSlot(); break
            case "screenshot": emu.screenshot(); break
            case "palette": emu.cyclePalette(); break
            case "reset": emu.reset(); toast.show("reset"); break
            case "fullscreen": keys.toggleFullscreen(); break
            case "mute":
                emu.muted = !emu.muted
                toast.show(emu.muted ? "muted" : "unmuted")
                break
            }
        }
        function onToast(message) { toast.show(message) }
    }

    Connections {
        target: emu
        function onErrorChanged() {
            if (emu.lastError.length > 0)
                toast.show(emu.lastError)
        }
        function onPaletteModeChanged() {
            toast.show("palette · " + emu.paletteName)
        }
        function onToast(message) {
            toast.show(message)
        }
    }

    // Auto-pause when the window loses focus; resume when it comes back
    // (only if the pause was ours).
    property bool autoPaused: false
    onActiveChanged: {
        if (!active && emu.pauseOnFocusLoss && emu.romLoaded && !emu.paused) {
            emu.togglePause()
            autoPaused = true
        } else if (active && autoPaused) {
            if (emu.paused)
                emu.togglePause()
            autoPaused = false
        }
    }

    Item {
        id: keys
        focus: true

        function toggleFullscreen() {
            root.visibility = root.visibility === Window.FullScreen
                ? Window.Windowed : Window.FullScreen
        }

        Keys.onPressed: (event) => {
            // Fixed application keys.
            if (!event.isAutoRepeat) {
                switch (event.key) {
                case Qt.Key_Escape: browser.toggle(); event.accepted = true; return
                case Qt.Key_F1: help.toggle(); event.accepted = true; return
                case Qt.Key_F2: settings.toggle(); event.accepted = true; return
                case Qt.Key_F11: toggleFullscreen(); event.accepted = true; return
                case Qt.Key_Plus:
                case Qt.Key_Equal:
                    emu.volume = Math.min(1, emu.volume + 0.1)
                    toast.show("volume " + Math.round(emu.volume * 100) + "%")
                    event.accepted = true; return
                case Qt.Key_Minus:
                    emu.volume = Math.max(0, emu.volume - 0.1)
                    toast.show("volume " + Math.round(emu.volume * 100) + "%")
                    event.accepted = true; return
                }
            }

            const act = input.actionForKey(event.key)
            if (act === "") return

            const bit = input.buttonBit(act)
            if (bit >= 0) {
                if (!event.isAutoRepeat) emu.setButton(bit, true)
                event.accepted = true
                return
            }
            if (event.isAutoRepeat) return
            switch (act) {
            case "turbo": emu.turbo = true; break
            case "pause": emu.togglePause(); break
            case "save_state": emu.saveState(); break
            case "load_state": emu.loadState(); break
            case "next_slot": emu.nextSlot(); break
            case "screenshot": emu.screenshot(); break
            case "palette": emu.cyclePalette(); break
            case "reset": emu.reset(); toast.show("reset"); break
            case "fullscreen": toggleFullscreen(); break
            case "mute":
                emu.muted = !emu.muted
                toast.show(emu.muted ? "muted" : "unmuted")
                break
            default: return
            }
            event.accepted = true
        }

        Keys.onReleased: (event) => {
            if (event.isAutoRepeat) return
            const act = input.actionForKey(event.key)
            if (act === "") return
            const bit = input.buttonBit(act)
            if (bit >= 0) {
                emu.setButton(bit, false)
                event.accepted = true
            } else if (act === "turbo") {
                emu.turbo = false
                event.accepted = true
            }
        }
    }

    Component.onCompleted: {
        keys.forceActiveFocus()
        // A pad opened before QML loaded emitted its toast into the void.
        if (pad.connected)
            toast.show("controller · " + pad.name.toLowerCase())
        if (Qt.application.arguments.indexOf("--open-settings") >= 0)
            settings.open()
        else if (!emu.romLoaded)
            browser.open()
    }
}
