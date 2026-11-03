import QtQuick
import QtQuick.Controls
import QtQuick.Window
import Brainroom 1.0

ApplicationWindow {
    id: root
    visible: true
    width: 1200
    height: 800
    minimumWidth: 640
    minimumHeight: 480
    title: "Brainroom"
    color: "#141414"

    AppController {
        id: app
    }

    Component.onCompleted: app.boot()

    Column {
        anchors.centerIn: parent
        spacing: 12

        Text {
            text: "Brainroom"
            color: "#e6e6e6"
            font.pixelSize: 36
            font.bold: true
        }

        Text {
            text: app.status
            color: "#8d8d8d"
            font.pixelSize: 16
        }

        Text {
            text: "v" + app.version + " · based on LearnHarness (AGPL-3.0)"
            color: "#5a5a6a"
            font.pixelSize: 12
        }
    }
}
