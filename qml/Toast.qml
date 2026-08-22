import QtQuick

Rectangle {
    id: toast
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 52
    width: label.width + 28
    height: 30
    radius: 6
    color: theme.darkBackground
    border.width: 1
    border.color: theme.lighterBackground
    opacity: 0
    visible: opacity > 0

    function show(text) {
        label.text = text
        opacity = 1
        hideTimer.restart()
    }

    Text {
        id: label
        anchors.centerIn: parent
        font.family: monoFont
        font.pixelSize: 12
        color: theme.foreground
    }

    Timer {
        id: hideTimer
        interval: 1800
        onTriggered: toast.opacity = 0
    }

    Behavior on opacity { NumberAnimation { duration: 150 } }
}
