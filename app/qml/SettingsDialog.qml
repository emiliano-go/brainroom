import QtQuick
import QtQuick.Controls

Item {
    id: dialog

    property var theme
    signal closed()

    Rectangle {
        anchors.fill: parent
        color: "#00000099"
        MouseArea {
            anchors.fill: parent
            onClicked: dialog.closed()
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: 920
        height: 620
        radius: 14
        color: "#1a1a1a"
        border.color: "#343434"
        border.width: 1

        MouseArea { anchors.fill: parent }

        Text {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.leftMargin: 28
            anchors.topMargin: 24
            text: "Settings"
            color: dialog.theme.fg
            font.family: dialog.theme.titleFont
            font.pixelSize: 30
            font.bold: true
        }

        Rectangle {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.rightMargin: 22
            anchors.topMargin: 22
            width: 30
            height: 30
            radius: 6
            color: "transparent"
            border.color: dialog.theme.border
            border.width: 1
            Text {
                anchors.centerIn: parent
                text: "×"
                color: dialog.theme.fg
                font.pixelSize: 18
            }
            MouseArea {
                anchors.fill: parent
                onClicked: dialog.closed()
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.topMargin: 76
            width: 200
            color: "transparent"

            Column {
                anchors.left: parent.left
                anchors.leftMargin: 20
                spacing: 4
                Repeater {
                    model: ["General", "Providers", "Models", "Appearance", "Graph", "Keyboard"]
                    delegate: Rectangle {
                        width: 164
                        height: 34
                        radius: 7
                        color: index === 2 ? "#262626" : "transparent"
                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData
                            color: index === 2 ? dialog.theme.fg : dialog.theme.fgDim
                            font.family: dialog.theme.proseFont
                            font.pixelSize: 14
                        }
                    }
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.leftMargin: 220
            anchors.right: parent.right
            anchors.rightMargin: 28
            anchors.top: parent.top
            anchors.topMargin: 86
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 28
            color: "transparent"

            Column {
                width: parent.width
                spacing: 18

                Text {
                    text: "Model slots"
                    color: dialog.theme.fg
                    font.family: dialog.theme.uiFont
                    font.pixelSize: 13
                    font.letterSpacing: 1.2
                }

                Repeater {
                    model: [
                        { label: "Strong", value: "nvidia/nemotron-3-ultra-550b-a55b" },
                        { label: "Cheap", value: "nvidia/nemotron-3.5-lightning:free" },
                        { label: "Vision", value: "anthropic/claude-haiku-5.5" }
                    ]
                    delegate: Rectangle {
                        width: parent.width
                        height: 52
                        radius: 8
                        color: "#202020"
                        border.color: "#303030"
                        border.width: 1
                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: dialog.theme.fg
                            font.family: dialog.theme.proseFont
                            font.pixelSize: 15
                        }
                        Text {
                            anchors.right: parent.right
                            anchors.rightMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.value
                            color: dialog.theme.fgDim
                            font.family: dialog.theme.uiFont
                            font.pixelSize: 11
                        }
                    }
                }

                Text {
                    text: "Teaching"
                    color: dialog.theme.fg
                    font.family: dialog.theme.uiFont
                    font.pixelSize: 13
                    font.letterSpacing: 1.2
                }

                Rectangle {
                    width: parent.width
                    height: 56
                    radius: 8
                    color: "#202020"
                    border.color: "#303030"
                    border.width: 1
                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Guided teaching"
                        color: dialog.theme.fg
                        font.family: dialog.theme.proseFont
                        font.pixelSize: 15
                    }
                    Rectangle {
                        anchors.right: parent.right
                        anchors.rightMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 22
                        radius: 11
                        color: "#2f2f2f"
                        Rectangle {
                            x: 3
                            y: 3
                            width: 16
                            height: 16
                            radius: 8
                            color: dialog.theme.fg
                        }
                    }
                }
            }
        }
    }
}
