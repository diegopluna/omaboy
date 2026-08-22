// Walker-style game picker: type to filter, arrows to move, enter to play.
import QtQuick
import QtQuick.Dialogs

Item {
    id: browser
    visible: false

    signal closed()
    signal requestHelp()
    signal requestSettings()

    function refocus() { filter.forceActiveFocus() }

    property var library: []
    property var entries: []

    function open() {
        library = emu.scanLibrary()
        filter.text = ""
        rebuild()
        visible = true
        filter.forceActiveFocus()
    }

    function close() {
        visible = false
        closed()
    }

    function toggle() { visible ? close() : open() }

    function rebuild() {
        const q = filter.text.toLowerCase()
        let list = []
        const recents = emu.recentRoms()
        if (q.length === 0) {
            for (const p of recents) {
                const name = p.split("/").pop().replace(/\.(gb|gbc|zip)$/i, "")
                list.push({ name: name, path: p, recent: true })
            }
        }
        for (const e of library) {
            if (q.length > 0 && e.name.toLowerCase().indexOf(q) < 0)
                continue
            if (q.length === 0 && recents.indexOf(e.path) >= 0)
                continue
            list.push({ name: e.name, path: e.path, recent: false })
        }
        entries = list
        listView.currentIndex = list.length > 0 ? 0 : -1
    }

    function activate() {
        if (listView.currentIndex < 0 || listView.currentIndex >= entries.length)
            return
        const e = entries[listView.currentIndex]
        if (emu.loadRom(e.path))
            close()
    }

    // Scrim
    Rectangle {
        anchors.fill: parent
        color: theme.darkerBackground
        opacity: 0.75
        MouseArea { anchors.fill: parent; onClicked: browser.close() }
    }

    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(620, parent.width - 80)
        readonly property int listHeight: entries.length > 0
            ? Math.min(entries.length * 30, Math.max(120, browser.height * 0.7 - 120))
            : 110
        height: searchRow.height + listHeight + 42
        color: theme.background
        border.width: 2
        border.color: theme.accent
        radius: 6

        Column {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            Row {
                id: searchRow
                width: parent.width
                spacing: 8

                Text {
                    text: "❯"
                    anchors.verticalCenter: parent.verticalCenter
                    font.family: monoFont
                    font.pixelSize: 16
                    color: theme.accent
                }

                TextInput {
                    id: filter
                    width: parent.width - 24
                    font.family: monoFont
                    font.pixelSize: 16
                    color: theme.foreground
                    clip: true
                    onTextChanged: browser.rebuild()

                    Text {
                        visible: filter.text.length === 0
                        text: "search games…"
                        font: filter.font
                        color: theme.mutedColor
                    }

                    Keys.onPressed: (event) => {
                        switch (event.key) {
                        case Qt.Key_Escape:
                            browser.close(); break
                        case Qt.Key_F1:
                            browser.requestHelp(); break
                        case Qt.Key_F2:
                            browser.requestSettings(); break
                        case Qt.Key_Down:
                            listView.currentIndex =
                                Math.min(listView.currentIndex + 1, entries.length - 1)
                            break
                        case Qt.Key_Up:
                            listView.currentIndex = Math.max(listView.currentIndex - 1, 0)
                            break
                        case Qt.Key_Return:
                        case Qt.Key_Enter:
                            browser.activate(); break
                        case Qt.Key_O:
                            if (event.modifiers & Qt.ControlModifier) {
                                fileDialog.open(); break
                            }
                            return
                        case Qt.Key_D:
                            if (event.modifiers & Qt.ControlModifier) {
                                dirDialog.open(); break
                            }
                            return
                        default:
                            return
                        }
                        event.accepted = true
                    }
                }
            }

            Rectangle { width: parent.width; height: 1; color: theme.lighterBackground }

            ListView {
                id: listView
                width: parent.width
                height: panel.listHeight - 2
                clip: true
                model: entries
                highlightMoveDuration: 60

                delegate: Rectangle {
                    width: listView.width
                    height: 30
                    radius: 4
                    color: ListView.isCurrentItem ? theme.selection : "transparent"

                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 10
                        spacing: 8
                        Text {
                            text: modelData.recent ? "↺" : "▸"
                            font.family: monoFont
                            font.pixelSize: 13
                            color: modelData.recent ? theme.yellow : theme.mutedColor
                        }
                        Text {
                            text: modelData.name
                            font.family: monoFont
                            font.pixelSize: 13
                            color: theme.foreground
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        onEntered: listView.currentIndex = index
                        onClicked: browser.activate()
                    }
                }

                Text {
                    visible: entries.length === 0
                    anchors.centerIn: parent
                    width: parent.width - 40
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    text: filter.text.length > 0
                          ? "nothing matches"
                          : "no games found in " + emu.libraryDir()
                            + "\nctrl+o to open a rom · ctrl+d to set the library folder"
                    font.family: monoFont
                    font.pixelSize: 12
                    color: theme.mutedColor
                    lineHeight: 1.4
                }
            }

        }
    }

    FileDialog {
        id: fileDialog
        title: "open rom"
        nameFilters: ["Game Boy ROMs (*.gb *.gbc *.zip)"]
        onAccepted: {
            if (emu.loadRom(selectedFile.toString()))
                browser.close()
        }
    }

    FolderDialog {
        id: dirDialog
        title: "set library folder"
        onAccepted: {
            emu.setLibraryDir(selectedFolder.toString())
            library = emu.scanLibrary()
            rebuild()
        }
    }
}
