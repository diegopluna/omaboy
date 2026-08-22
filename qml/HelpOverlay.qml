import QtQuick

Item {
    id: help
    visible: false

    signal closed()

    function toggle() {
        visible = !visible
        if (!visible) closed()
    }

    // Rebuilt each time it opens so rebound keys show correctly.
    property var binds: []
    property string padSummary: ""
    function kn(id) { return input.keyName(id) }
    function buildBinds() {
        return [
            [kn("up") + " " + kn("down") + " " + kn("left") + " " + kn("right"), "d-pad"],
            [kn("a") + " / " + kn("b"), "a / b"],
            [kn("start"), "start"],
            [kn("select"), "select"],
            [kn("turbo"), "turbo (hold)"],
            [kn("pause"), "pause"],
            [kn("save_state") + " / " + kn("load_state"), "save / load state"],
            [kn("next_slot"), "next state slot"],
            [kn("screenshot"), "screenshot"],
            ["esc", "game library"],
            ["f2", "settings"],
            [kn("reset"), "reset"],
            [kn("palette"), "cycle palette"],
            [kn("mute") + " · + / -", "mute · volume"],
            [kn("fullscreen") + " · f11", "fullscreen"]
        ]
    }

    Rectangle {
        anchors.fill: parent
        color: theme.darkerBackground
        opacity: 0.75
        MouseArea { anchors.fill: parent; onClicked: help.toggle() }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(680, parent.width - 60)
        height: grid.height + title.height + padLine.height + 66
        color: theme.background
        border.width: 2
        border.color: theme.accent
        radius: 6

        Text {
            id: title
            anchors.top: parent.top
            anchors.topMargin: 16
            anchors.horizontalCenter: parent.horizontalCenter
            text: "keybindings"
            font.family: monoFont
            font.pixelSize: 15
            font.bold: true
            color: theme.accent
        }

        Grid {
            id: grid
            anchors.top: title.bottom
            anchors.topMargin: 16
            anchors.horizontalCenter: parent.horizontalCenter
            columns: 2
            columnSpacing: 28
            rowSpacing: 7

            Repeater {
                model: help.binds
                delegate: Row {
                    spacing: 12
                    Text {
                        width: 130
                        horizontalAlignment: Text.AlignRight
                        text: modelData[0]
                        font.family: monoFont
                        font.pixelSize: 13
                        color: theme.accent
                    }
                    Text {
                        text: modelData[1]
                        font.family: monoFont
                        font.pixelSize: 13
                        color: theme.foreground
                    }
                }
            }
        }

        Text {
            id: padLine
            anchors.top: grid.bottom
            anchors.topMargin: 14
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width - 40
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: (pad.connected ? pad.name.toLowerCase() : "controller")
                  + " · " + help.padSummary
            font.family: monoFont
            font.pixelSize: 12
            color: pad.connected ? theme.foreground : theme.mutedColor
        }
    }

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_F1) {
            toggle()
            event.accepted = true
        }
    }
    onVisibleChanged: {
        if (visible) {
            binds = buildBinds()
            padSummary = pad.summary()
            forceActiveFocus()
        }
    }
}
