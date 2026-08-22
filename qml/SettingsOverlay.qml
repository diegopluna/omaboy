// Keyboard-driven settings: quality-of-life toggles and control rebinding.
import QtQuick

Item {
    id: overlay
    visible: false

    signal closed()

    property var rows: []
    property string capturing: ""     // action id being rebound (keyboard)
    property string capturingPad: ""  // action id being rebound (controller)
    property int tick: 0              // bumped to refresh value bindings

    function open() {
        rows = buildModel()
        list.currentIndex = 1
        visible = true
        forceActiveFocus()
    }

    function close() {
        capturing = ""
        stopPadCapture()
        visible = false
        closed()
    }

    function stopPadCapture() {
        capturingPad = ""
        pad.capturing = false
    }

    function toggle() { visible ? close() : open() }

    function buildModel() {
        let r = []
        r.push({ type: "header", label: "options" })
        r.push({ type: "toggle", label: "pause when window unfocused", prop: "pauseOnFocusLoss" })
        r.push({ type: "toggle", label: "integer scaling (crisp pixels)", prop: "integerScaling" })
        r.push({ type: "toggle", label: "gbc color correction", prop: "colorCorrection" })
        r.push({ type: "choice", label: "turbo speed", prop: "turboSpeed",
                 values: [2, 4, 8], names: ["2×", "4×", "8×"] })
        r.push({ type: "choice", label: "save state slot", prop: "stateSlot",
                 values: [1, 2, 3], names: ["1", "2", "3"] })
        r.push({ type: "choice", label: "dmg palette", prop: "paletteMode",
                 values: [1, 2, 0], names: ["classic", "mono", "omarchy"] })
        r.push({ type: "volume", label: "volume" })
        r.push({ type: "toggle", label: "resume last game on launch", prop: "autoLoadLast" })
        r.push({ type: "toggle", label: "show fps", prop: "showFps" })
        r.push({ type: "header", label: "controls · enter to rebind" })
        const bindings = input.model()
        for (const b of bindings)
            r.push({ type: "key", label: b.label, id: b.id })
        r.push({ type: "header", label: "controller · enter to rebind" })
        const padBindings = pad.model()
        for (const b of padBindings)
            r.push({ type: "pad", label: b.label, id: b.id })
        r.push({ type: "action", label: "reset controls to defaults" })
        return r
    }

    function valueText(row) {
        tick // dependency: re-evaluate on option changes
        if (row.type === "toggle")
            return emu[row.prop] ? "on" : "off"
        if (row.type === "choice") {
            const i = row.values.indexOf(emu[row.prop])
            return row.names[Math.max(0, i)]
        }
        if (row.type === "key")
            return capturing === row.id ? "press a key…" : input.keyName(row.id)
        if (row.type === "pad")
            return capturingPad === row.id ? "press a button…" : pad.padName(row.id)
        if (row.type === "volume")
            return emu.muted ? "muted" : Math.round(emu.volume * 100) + "%"
        return ""
    }

    function adjust(row, dir) {
        if (row.type === "toggle") {
            emu[row.prop] = !emu[row.prop]
        } else if (row.type === "choice") {
            const n = row.values.length
            const i = (row.values.indexOf(emu[row.prop]) + dir + n) % n
            emu[row.prop] = row.values[i]
        } else if (row.type === "volume") {
            emu.muted = false
            emu.volume = Math.max(0, Math.min(1, emu.volume + dir * 0.1))
        } else if (row.type === "key") {
            stopPadCapture()
            capturing = capturing === row.id ? "" : row.id
        } else if (row.type === "pad") {
            capturing = ""
            if (capturingPad === row.id) {
                stopPadCapture()
            } else {
                capturingPad = row.id
                pad.capturing = true
            }
        } else if (row.type === "action") {
            input.resetDefaults()
            pad.resetDefaults()
        }
    }

    function move(dir) {
        let i = list.currentIndex
        for (let step = 0; step < rows.length; step++) {
            i = (i + dir + rows.length) % rows.length
            if (rows[i].type !== "header") break
        }
        list.currentIndex = i
    }

    Connections {
        target: emu
        function onOptionsChanged() { overlay.tick++ }
        function onPaletteModeChanged() { overlay.tick++ }
        function onVolumeChanged() { overlay.tick++ }
    }
    Connections {
        target: input
        function onChanged() { overlay.tick++ }
    }
    Connections {
        target: pad
        function onChanged() { overlay.tick++ }
        function onCaptured(inputId) {
            if (overlay.capturingPad === "")
                return
            pad.rebind(overlay.capturingPad, inputId)
            overlay.stopPadCapture()
        }
    }

    Rectangle {
        anchors.fill: parent
        color: theme.darkerBackground
        opacity: 0.75
        MouseArea { anchors.fill: parent; onClicked: overlay.close() }
    }

    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(560, parent.width - 80)
        height: Math.min(title.height + list.contentHeight + hints.height + 58,
                         parent.height - 80)
        color: theme.background
        border.width: 2
        border.color: theme.accent
        radius: 6

        Text {
            id: title
            anchors.top: parent.top
            anchors.topMargin: 14
            anchors.horizontalCenter: parent.horizontalCenter
            text: "settings"
            font.family: monoFont
            font.pixelSize: 15
            font.bold: true
            color: theme.accent
        }

        ListView {
            id: list
            anchors.top: title.bottom
            anchors.topMargin: 10
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.bottom: hints.top
            anchors.bottomMargin: 8
            clip: true
            model: rows
            highlightMoveDuration: 60
            keyNavigationEnabled: false

            delegate: Rectangle {
                width: list.width
                height: modelData.type === "header" ? 30 : 28
                radius: 4
                color: modelData.type !== "header" && ListView.isCurrentItem
                       ? theme.selection : "transparent"

                Text {
                    visible: modelData.type === "header"
                    anchors.left: parent.left
                    anchors.leftMargin: 4
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 5
                    text: modelData.label
                    font.family: monoFont
                    font.pixelSize: 11
                    font.bold: true
                    color: theme.mutedColor
                }

                Text {
                    visible: modelData.type !== "header"
                    anchors.left: parent.left
                    anchors.leftMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label
                    font.family: monoFont
                    font.pixelSize: 13
                    color: modelData.type === "action" ? theme.yellow : theme.foreground
                }

                Text {
                    visible: modelData.type !== "header" && modelData.type !== "action"
                    anchors.right: parent.right
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    text: overlay.valueText(modelData)
                    font.family: monoFont
                    font.pixelSize: 13
                    color: (overlay.capturing === modelData.id && modelData.type === "key")
                           || (overlay.capturingPad === modelData.id && modelData.type === "pad")
                           ? theme.yellow : theme.accent
                }

                MouseArea {
                    anchors.fill: parent
                    enabled: modelData.type !== "header"
                    hoverEnabled: true
                    onEntered: list.currentIndex = index
                    onClicked: overlay.adjust(modelData, 1)
                }
            }
        }

        Text {
            id: hints
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 12
            anchors.horizontalCenter: parent.horizontalCenter
            text: capturing !== "" ? "press the new key · esc cancel"
                : capturingPad !== "" ? "press a controller button · esc cancel"
                : "↑↓ move · ←→/enter change · esc close"
            font.family: monoFont
            font.pixelSize: 11
            color: theme.mutedColor
        }
    }

    Keys.onPressed: (event) => {
        event.accepted = true
        if (capturingPad !== "") {
            if (event.key === Qt.Key_Escape)
                stopPadCapture()
            return
        }
        if (capturing !== "") {
            if (event.key === Qt.Key_Escape) {
                capturing = ""
                return
            }
            // Ignore lone modifier "press" only for keys Qt reports as unknown.
            if (event.key === 0 || event.key === Qt.Key_unknown) return
            if ([Qt.Key_Escape, Qt.Key_F1, Qt.Key_F2, Qt.Key_F11].indexOf(event.key) >= 0) {
                toast.show("that key is reserved")
                capturing = ""
                return
            }
            input.rebind(capturing, event.key)
            capturing = ""
            return
        }
        switch (event.key) {
        case Qt.Key_Escape:
        case Qt.Key_F2:
            close(); break
        case Qt.Key_Up: move(-1); break
        case Qt.Key_Down: move(1); break
        case Qt.Key_Left: adjust(rows[list.currentIndex], -1); break
        case Qt.Key_Right: adjust(rows[list.currentIndex], 1); break
        case Qt.Key_Return:
        case Qt.Key_Enter:
        case Qt.Key_Space:
            adjust(rows[list.currentIndex], 1); break
        default:
            event.accepted = false
        }
    }
}
