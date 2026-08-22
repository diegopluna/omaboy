// Omaboy game picker: recents + ROM library, one click to play.
// The emulator itself is the omaboy application (built from this repo);
// the panel detects it and offers the library, or install pointers if absent.
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "io.github.diegopluna.omaboy"
  ipcTarget: "io.github.diegopluna.omaboy"
  manageIpc: false

  property var anchorItem: null
  property bool openedFromHotkey: false
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  // ---- state ----
  property bool appInstalled: false
  property bool checkedInstall: false
  property var recentGames: []   // [{name, path}]
  property var libraryGames: []  // [{name, path}]

  function open() {
    openedFromHotkey = false
    root.controller.show()
    refresh()
  }

  function openFromHotkey() {
    openedFromHotkey = true
    root.controller.show()
    refresh()
  }

  function close() {
    root.controller.hide()
  }

  function toggle() {
    if (root.opened) root.close()
    else root.openFromHotkey()
  }

  function switchPanel(direction) {
    if (root.bar && typeof root.bar.switchPanelFrom === "function")
      return root.bar.switchPanelFrom(root.barIdentity, direction)
    return false
  }

  function refresh() {
    checkProc.running = true
    configFile.reload()
    scanProc.running = true
  }

  function launchApp(romPath) {
    if (!root.appInstalled) {
      root.controller.show()
      return
    }
    var cmd = ["omaboy"]
    if (romPath !== "") cmd.push(romPath)
    Quickshell.execDetached(cmd)
    root.close()
  }

  function baseName(path) {
    var slash = path.lastIndexOf("/")
    var name = slash >= 0 ? path.substring(slash + 1) : path
    return name.replace(/\.(gb|gbc|zip)$/i, "")
  }

  // ---- omaboy binary detection ----
  Process {
    id: checkProc
    command: ["sh", "-c", "command -v omaboy"]
    onExited: function(code) {
      root.appInstalled = code === 0
      root.checkedInstall = true
    }
  }

  // ---- recents + library dir from omaboy's own config ----
  property string libraryDir: Quickshell.env("HOME") + "/Games"

  FileView {
    id: configFile
    path: Quickshell.env("HOME") + "/.config/omaboy/omaboy.conf"
    printErrors: false
    onLoaded: root.parseConfig(text())
    onLoadFailed: { root.recentGames = [] }
  }

  function parseConfig(text) {
    var recents = []
    var lines = text.split("\n")
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i]
      if (line.indexOf("recentRoms=") === 0) {
        var parts = line.substring("recentRoms=".length).split(", ")
        for (var j = 0; j < parts.length && recents.length < 8; j++) {
          var p = parts[j].trim()
          if (p !== "") recents.push({ name: baseName(p), path: p })
        }
      } else if (line.indexOf("libraryDir=") === 0) {
        var dir = line.substring("libraryDir=".length).trim()
        if (dir !== "") root.libraryDir = dir
      }
    }
    root.recentGames = recents
  }

  // ---- library scan ----
  Process {
    id: scanProc
    command: ["sh", "-c",
      "find \"" + root.libraryDir + "\" \"$HOME/ROMs\" \"$HOME/roms\" -maxdepth 3 -type f " +
      "\\( -iname '*.gb' -o -iname '*.gbc' -o -iname '*.zip' \\) 2>/dev/null | sort | head -100"]
    stdout: StdioCollector {
      onStreamFinished: {
        var games = []
        var seen = {}
        var lines = text.split("\n")
        for (var i = 0; i < lines.length; i++) {
          var p = lines[i].trim()
          if (p === "" || seen[p]) continue
          seen[p] = true
          games.push({ name: root.baseName(p), path: p })
        }
        root.libraryGames = games
      }
    }
  }

  // Hotkey/IPC summon: omarchy shell io.github.diegopluna.omaboy toggle
  IpcHandler {
    target: root.ipcTarget

    function open(): void { root.openFromHotkey() }
    function close(): void { root.close() }
    function show(): void { root.openFromHotkey() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(contentColumn.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: contentColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: contentColumn
          width: parent.width
          spacing: Style.space(10)

          PanelHero {
            width: parent.width
            title: "Omaboy"
            meta: root.appInstalled
              ? "game boy · game boy color"
              : "emulator not installed"
          }

          // Not-installed pointer
          Column {
            width: parent.width
            spacing: Style.space(6)
            visible: root.checkedInstall && !root.appInstalled

            Text {
              width: parent.width
              wrapMode: Text.WordWrap
              text: "The omaboy emulator isn't on your PATH. Build it from the repo this plugin ships in:"
              color: Color.foreground
              font.family: root.bar ? root.bar.fontFamily : "monospace"
              font.pixelSize: Style.font.caption
            }
            Button {
              text: "Open build instructions"
              bordered: true
              hasCursor: true
              onClicked: {
                Quickshell.execDetached(["xdg-open", "https://github.com/diegopluna/omaboy#build--install"])
                root.close()
              }
            }
          }

          // Recents
          PanelSectionHeader {
            width: parent.width
            visible: root.appInstalled && root.recentGames.length > 0
            text: "Recent"
          }
          Repeater {
            model: root.appInstalled ? root.recentGames : []
            delegate: gameRow
          }

          // Library
          PanelSectionHeader {
            width: parent.width
            visible: root.appInstalled && root.libraryGames.length > 0
            text: "Library"
          }
          Repeater {
            model: root.appInstalled ? root.libraryGames : []
            delegate: gameRow
          }

          Text {
            width: parent.width
            visible: root.appInstalled && root.libraryGames.length === 0 && root.recentGames.length === 0
            wrapMode: Text.WordWrap
            text: "No games found. Drop .gb / .gbc files in ~/Games, or open omaboy and press esc to pick a folder."
            color: Color.foreground
            opacity: 0.7
            font.family: root.bar ? root.bar.fontFamily : "monospace"
            font.pixelSize: Style.font.caption
          }

          Button {
            visible: root.appInstalled
            text: "Open omaboy"
            bordered: true
            hasCursor: true
            onClicked: root.launchApp("")
          }
        }
      }
    }
  }

  Component {
    id: gameRow

    Rectangle {
      required property var modelData
      width: contentColumn.width
      implicitHeight: rowText.implicitHeight + Style.space(10)
      radius: Style.cornerRadius
      color: rowMouse.containsMouse ? Style.hoverFillFor(Color.foreground, Color.accent) : "transparent"

      Text {
        id: rowText
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: Style.space(8)
        anchors.right: parent.right
        anchors.rightMargin: Style.space(8)
        elide: Text.ElideRight
        text: "▸ " + modelData.name
        color: Color.foreground
        font.family: root.bar ? root.bar.fontFamily : "monospace"
        font.pixelSize: Style.font.body
      }

      MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.launchApp(modelData.path)
      }
    }
  }
}
