import QtQuick
import QtQuick.Controls
import QtQuick.Window

Item {
    id: home

    property var theme
    signal openSession(string title)
    signal openSettings()
    signal toggleSidebar()
    signal closeWindow()
    signal minimizeWindow()
    signal maximizeWindow()

    GraphCanvas {
        anchors.fill: parent
        theme: home.theme
    }

    MouseArea {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 64
        acceptedButtons: Qt.LeftButton
        onPressed: Window.window.startSystemMove()
    }

    Item {
        id: topBar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 64

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 22
            anchors.verticalCenter: parent.verticalCenter
            spacing: 9
            Repeater {
                model: [
                    { c: "#ec6a5e", a: "close" },
                    { c: "#f4bf4f", a: "min" },
                    { c: "#61c554", a: "max" }
                ]
                delegate: Rectangle {
                    width: 12
                    height: 12
                    radius: 6
                    color: modelData.c
                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            if (modelData.a === "close") home.closeWindow();
                            else if (modelData.a === "min") home.minimizeWindow();
                            else home.maximizeWindow();
                        }
                    }
                }
            }
        }

        Rectangle {
            id: sidebarButton
            anchors.left: parent.left
            anchors.leftMargin: 252
            anchors.verticalCenter: parent.verticalCenter
            width: 24
            height: 24
            radius: 5
            color: "transparent"
            border.color: home.theme.fgDim
            border.width: 1
            Rectangle {
                anchors.left: parent.left
                anchors.leftMargin: 6
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 1
                color: home.theme.fgDim
            }
            MouseArea {
                anchors.fill: parent
                onClicked: home.toggleSidebar()
            }
        }

        Rectangle {
            id: search
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.verticalCenter: parent.verticalCenter
            width: 386
            height: 40
            radius: 10
            color: "#1b1b1b"
            border.color: "#2e2e2e"
            border.width: 1
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                text: "⌕"
                color: home.theme.fgDim
                font.pixelSize: 18
            }
            TextInput {
                anchors.left: parent.left
                anchors.leftMargin: 40
                anchors.right: parent.right
                anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                color: home.theme.fg
                font.family: home.theme.proseFont
                font.pixelSize: 14
                clip: true
                Text {
                    visible: parent.text.length === 0
                    text: "Search nodes…"
                    color: home.theme.fgFaint
                    font: parent.font
                }
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 22
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            Rectangle {
                width: 112
                height: 40
                radius: 10
                color: "transparent"
                border.color: home.theme.border
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 7
                    Text { text: "☰"; color: home.theme.fg; font.pixelSize: 14 }
                    Text { text: "Chains"; color: home.theme.fg; font.family: home.theme.uiFont; font.pixelSize: 12 }
                }
            }

            Rectangle {
                width: 150
                height: 40
                radius: 10
                color: "transparent"
                border.color: home.theme.border
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 7
                    Text { text: "↥"; color: home.theme.fg; font.pixelSize: 16 }
                    Text { text: "Import notes…"; color: home.theme.fg; font.family: home.theme.uiFont; font.pixelSize: 12 }
                }
            }

            Rectangle {
                width: 40
                height: 40
                radius: 10
                color: "transparent"
                border.color: home.theme.border
                border.width: 1
                Text {
                    anchors.centerIn: parent
                    text: "⚙"
                    color: home.theme.fg
                    font.pixelSize: 16
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: home.openSettings()
                }
            }
        }

        Text {
            anchors.right: parent.right
            anchors.rightMargin: 22
            anchors.top: parent.top
            anchors.topMargin: 52
            text: "$0.49"
            color: home.theme.fgDim
            font.family: home.theme.uiFont
            font.pixelSize: 12
        }
    }

    Item {
        id: composer
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 28
        width: Math.min(parent.width - 140, 1060)
        height: 136

        Rectangle {
            id: inputShell
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 62
            radius: 12
            color: "#1b1b1b"
            border.color: "#2e2e2e"
            border.width: 1

            TextInput {
                id: promptInput
                anchors.left: parent.left
                anchors.leftMargin: 22
                anchors.right: sendButton.left
                anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                color: home.theme.fg
                selectionColor: home.theme.border
                font.family: home.theme.proseFont
                font.pixelSize: 17
                clip: true
                Text {
                    visible: parent.text.length === 0
                    text: "What do you want to learn?"
                    color: home.theme.fgFaint
                    font: parent.font
                }
                onAccepted: home.openSession("Gradient descent")
            }

            Rectangle {
                id: sendButton
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                width: 44
                height: 44
                radius: 8
                color: "#2a2a2a"
                Text {
                    anchors.centerIn: parent
                    text: "↲"
                    color: home.theme.fg
                    font.pixelSize: 20
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: home.openSession("Gradient descent")
                }
            }
        }

        Row {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: inputShell.bottom
            anchors.topMargin: 10
            spacing: 10

            Rectangle {
                width: 166
                height: 34
                radius: 8
                color: "#1b1b1b"
                border.color: "#2e2e2e"
                border.width: 1
                Row {
                    anchors.fill: parent
                    anchors.margins: 3
                    spacing: 2
                    Rectangle {
                        width: 72
                        height: 28
                        radius: 6
                        color: "#2a2a2a"
                        Text {
                            anchors.centerIn: parent
                            text: "TEACH"
                            color: home.theme.fg
                            font.family: home.theme.uiFont
                            font.pixelSize: 10
                            font.letterSpacing: 1
                        }
                    }
                    Rectangle {
                        width: 84
                        height: 28
                        radius: 6
                        color: "transparent"
                        Row {
                            anchors.centerIn: parent
                            spacing: 4
                            Text { text: "⚗"; color: home.theme.fgDim; font.pixelSize: 12 }
                            Text {
                                text: "PRACTICE"
                                color: home.theme.fgDim
                                font.family: home.theme.uiFont
                                font.pixelSize: 10
                                font.letterSpacing: 0.6
                            }
                        }
                    }
                }
            }

            Rectangle {
                width: parent.width - 166 - 10 - 120 - 10 - 86 - 10 - 72
                height: 34
                radius: 8
                color: "#1b1b1b"
                border.color: "#2e2e2e"
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 7
                    Text { text: "◉"; color: home.theme.fgDim; font.pixelSize: 13 }
                    Text { text: "Dictate"; color: home.theme.fgDim; font.family: home.theme.proseFont; font.pixelSize: 13 }
                }
            }

            Rectangle {
                width: 120
                height: 34
                radius: 8
                color: "#1b1b1b"
                border.color: "#2e2e2e"
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "gpt-6.1-sol"; color: home.theme.fg; font.family: home.theme.uiFont; font.pixelSize: 11 }
                    Text { text: "⌄"; color: home.theme.fgDim; font.pixelSize: 11 }
                }
            }

            Rectangle {
                width: 72
                height: 34
                radius: 8
                color: "#1b1b1b"
                border.color: "#2e2e2e"
                border.width: 1
                Row {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "max"; color: home.theme.fg; font.family: home.theme.uiFont; font.pixelSize: 11 }
                    Text { text: "⌄"; color: home.theme.fgDim; font.pixelSize: 11 }
                }
            }
        }
    }
}
