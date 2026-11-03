import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Rectangle {
    id: session

    property var theme
    property string lessonTitle: "Gradient descent"
    property string streamText: ""
    property string errorText: ""
    property bool busy: false

    signal openSettings()
    signal toggleSidebar()

    color: theme.bg

    MouseArea {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 70
        acceptedButtons: Qt.LeftButton
        onPressed: Window.window.startSystemMove()
    }

    Item {
        id: topBar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 72

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 48
            anchors.verticalCenter: parent.verticalCenter
            text: session.lessonTitle
            color: session.theme.fg
            font.family: session.theme.titleFont
            font.pixelSize: 38
            font.bold: true
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 30
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            Rectangle {
                width: 92
                height: 34
                radius: 8
                color: "transparent"
                border.color: session.theme.border
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "▤"; color: session.theme.fg; font.pixelSize: 12 }
                    Text {
                        text: "PLAN"
                        color: session.theme.fg
                        font.family: session.theme.uiFont
                        font.pixelSize: 10
                        font.letterSpacing: 1.4
                    }
                }
            }

            Rectangle {
                width: 126
                height: 34
                radius: 8
                color: "transparent"
                border.color: session.theme.border
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 7
                    Rectangle {
                        width: 12
                        height: 12
                        radius: 6
                        color: "transparent"
                        border.color: session.theme.fgDim
                        border.width: 1
                    }
                    Text {
                        text: "TEACHING"
                        color: session.theme.fg
                        font.family: session.theme.uiFont
                        font.pixelSize: 10
                        font.letterSpacing: 1.4
                    }
                }
            }
        }
    }

    PlanMinimap {
        anchors.right: parent.right
        anchors.rightMargin: -6
        anchors.bottom: stats.top
        anchors.bottomMargin: 70
        width: 250
        height: 230
        theme: session.theme
        z: 0
    }

    Flickable {
        id: scroller
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: topBar.bottom
        anchors.bottom: stats.top
        clip: true
        contentWidth: width
        contentHeight: contentColumn.implicitHeight + 80
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { }

        Column {
            id: contentColumn
            x: 48
            y: 8
            width: Math.min(scroller.width - 96, 820)
            spacing: 10

            Rectangle {
                visible: session.streamText !== "" || session.errorText !== "" || session.busy
                width: parent.width
                height: streamBody.implicitHeight + 26
                radius: 8
                color: "#151515"
                border.color: session.errorText !== "" ? "#5a2a2a" : "#2e2e2e"
                border.width: 1
                Text {
                    id: streamBody
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    text: session.errorText !== "" ? ("Error: " + session.errorText) : (session.streamText !== "" ? session.streamText : "Thinking…")
                    color: session.errorText !== "" ? session.theme.red : session.theme.fg
                    font.family: session.theme.proseFont
                    font.pixelSize: 15
                    lineHeight: 1.45
                    wrapMode: Text.WordWrap
                }
            }

            SampleLesson {
                width: parent.width
                theme: session.theme
                visible: session.streamText === "" && session.errorText === "" && !session.busy
            }
        }
    }

    Item {
        id: stats
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 46

        Rectangle {
            anchors.top: parent.top
            width: parent.width
            height: 1
            color: "#1f1f1f"
        }

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 48
            anchors.verticalCenter: parent.verticalCenter
            text: "83.7k / 1M · $0.1716"
            color: session.theme.fgDim
            font.family: session.theme.uiFont
            font.pixelSize: 11
        }

        Rectangle {
            anchors.left: parent.left
            anchors.leftMargin: 230
            anchors.verticalCenter: parent.verticalCenter
            width: 82
            height: 30
            radius: 8
            color: "#2a1717"
            border.color: "#5a2a2a"
            border.width: 1
            Row {
                anchors.centerIn: parent
                spacing: 6
                Rectangle { width: 8; height: 8; radius: 4; color: session.theme.red }
                Text { text: "Stop"; color: session.theme.fg; font.family: session.theme.proseFont; font.pixelSize: 13 }
            }
        }

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 332
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3
            Repeater {
                model: [3, 7, 12, 6, 16, 9, 18, 5, 14, 8, 19, 7, 12, 15, 6, 17, 9, 13, 5, 16, 8, 12, 6, 14]
                delegate: Rectangle {
                    width: 3
                    height: modelData
                    radius: 1
                    color: session.theme.fgFaint
                }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.rightMargin: 142
            anchors.verticalCenter: parent.verticalCenter
            width: 104
            height: 32
            radius: 8
            color: "#1b1b1b"
            border.color: "#2e2e2e"
            border.width: 1
            Row {
                anchors.centerIn: parent
                spacing: 5
                Text { text: "gpt-6.1-sol"; color: session.theme.fg; font.family: session.theme.uiFont; font.pixelSize: 10 }
                Text { text: "⌄"; color: session.theme.fgDim; font.pixelSize: 10 }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.rightMargin: 30
            anchors.verticalCenter: parent.verticalCenter
            width: 68
            height: 32
            radius: 8
            color: "#1b1b1b"
            border.color: "#2e2e2e"
            border.width: 1
            Row {
                anchors.centerIn: parent
                spacing: 5
                Text { text: "max"; color: session.theme.fg; font.family: session.theme.uiFont; font.pixelSize: 10 }
                Text { text: "⌄"; color: session.theme.fgDim; font.pixelSize: 10 }
            }
        }
    }
}
