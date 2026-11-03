import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Rectangle {
    id: sidebar
    color: theme.bg

    property var theme
    property string selectedTitle: "Gradient descent"
    property int tab: 0

    signal newTeaching()
    signal sessionSelected(string title)
    signal openSettings()
    signal goHome()
    signal closeWindow()
    signal minimizeWindow()
    signal maximizeWindow()

    Rectangle {
        anchors.right: parent.right
        width: 1
        height: parent.height
        color: "#1e1e1e"
    }

    MouseArea {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 58
        acceptedButtons: Qt.LeftButton
        onPressed: Window.window.startSystemMove()
    }

    Item {
        id: topBar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 58

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 18
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
                            if (modelData.a === "close") sidebar.closeWindow();
                            else if (modelData.a === "min") sidebar.minimizeWindow();
                            else sidebar.maximizeWindow();
                        }
                    }
                }
            }
        }

        Text {
            id: mark
            anchors.left: parent.left
            anchors.leftMargin: 102
            anchors.verticalCenter: parent.verticalCenter
            text: "◎"
            color: theme.fg
            font.pixelSize: 24
            MouseArea {
                anchors.fill: parent
                onClicked: sidebar.goHome()
            }
        }

        Item {
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.verticalCenter: parent.verticalCenter
            width: 22
            height: 22
            Rectangle {
                anchors.fill: parent
                radius: 4
                color: "transparent"
                border.color: theme.fgDim
                border.width: 1
            }
            Rectangle {
                anchors.left: parent.left
                anchors.leftMargin: 5
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 1
                color: theme.fgDim
            }
            MouseArea {
                anchors.fill: parent
                onClicked: sidebar.goHome()
            }
        }
    }

    ColumnLayout {
        anchors.top: topBar.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: 14
        anchors.rightMargin: 14
        anchors.topMargin: 4
        anchors.bottomMargin: 12
        spacing: 10

        Rectangle {
            Layout.fillWidth: true
            height: 34
            radius: 8
            color: "transparent"
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: "+  TEACHING"
                color: theme.fgDim
                font.family: theme.uiFont
                font.pixelSize: 12
                font.letterSpacing: 1.2
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                onClicked: sidebar.newTeaching()
                onEntered: parent.color = theme.panel
                onExited: parent.color = "transparent"
            }
        }

        Rectangle {
            Layout.fillWidth: true
            height: 30
            radius: 8
            color: "transparent"
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: "⚗  PRACTICE"
                color: theme.fgDim
                font.family: theme.uiFont
                font.pixelSize: 12
                font.letterSpacing: 1.2
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                onClicked: sidebar.tab = 1
                onEntered: parent.color = theme.panel
                onExited: parent.color = "transparent"
            }
        }

        Item { Layout.preferredHeight: 4 }

        Text {
            Layout.fillWidth: true
            text: "SESSIONS"
            color: theme.fgFaint
            font.family: theme.uiFont
            font.pixelSize: 11
            font.letterSpacing: 1.6
        }

        Rectangle {
            Layout.fillWidth: true
            height: 34
            radius: 8
            color: theme.panel
            border.color: theme.borderSoft
            border.width: 1

            Text {
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: "⌕"
                color: theme.fgFaint
                font.pixelSize: 16
            }
            TextInput {
                anchors.left: parent.left
                anchors.leftMargin: 32
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                color: theme.fg
                selectionColor: theme.border
                font.family: theme.proseFont
                font.pixelSize: 13
                clip: true
                Text {
                    visible: parent.text.length === 0
                    text: "Search…"
                    color: theme.fgFaint
                    font: parent.font
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            height: 32
            radius: 8
            color: theme.panel

            Row {
                anchors.fill: parent
                anchors.margins: 3
                spacing: 2
                Repeater {
                    model: ["TEACHING", "PRACTICE", "ALL"]
                    delegate: Rectangle {
                        width: (parent.width - 4) / 3
                        height: parent.height
                        radius: 6
                        color: sidebar.tab === index ? theme.panelHover : "transparent"
                        Text {
                            anchors.centerIn: parent
                            text: modelData
                            color: sidebar.tab === index ? theme.fg : theme.fgDim
                            font.family: theme.uiFont
                            font.pixelSize: 10
                            font.letterSpacing: 1.0
                        }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: sidebar.tab = index
                        }
                    }
                }
            }
        }

        ListView {
            id: sessions
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 2
            model: ListModel {
                ListElement { title: "Gradient descent"; status: "Waiting for your answer · just now" }
                ListElement { title: "Neural networks"; status: "Waiting for your answer · 1d ago" }
                ListElement { title: "Deflationary theories of truth"; status: "Waiting for your answer · 1d ago" }
                ListElement { title: "Relational model"; status: "Waiting for your answer · 2d ago" }
                ListElement { title: "Data structures and algorithms"; status: "3d ago" }
                ListElement { title: "Thermodynamic computing"; status: "3d ago" }
                ListElement { title: "Lean"; status: "Waiting for your answer · 5d ago" }
                ListElement { title: "Hume"; status: "Waiting for your answer · 6d ago" }
                ListElement { title: "Bayesian knowledge tracing"; status: "Waiting for your answer · 2026-10-08" }
                ListElement { title: "Lacan"; status: "Waiting for your answer · 2026-10-08" }
                ListElement { title: "Objet petit a"; status: "2026-10-08" }
                ListElement { title: "Transformers"; status: "2026-10-08" }
            }
            delegate: Rectangle {
                width: sessions.width
                height: 54
                radius: 8
                color: sidebar.selectedTitle === model.title ? "#202020" : "transparent"
                border.color: sidebar.selectedTitle === model.title ? "#303030" : "transparent"
                border.width: 1

                Column {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.right: parent.right
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 3
                    Text {
                        width: parent.width
                        text: model.title
                        elide: Text.ElideRight
                        color: theme.fg
                        font.family: theme.proseFont
                        font.pixelSize: 15
                    }
                    Text {
                        width: parent.width
                        text: model.status
                        elide: Text.ElideRight
                        color: theme.fgDim
                        font.family: theme.uiFont
                        font.pixelSize: 10
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: sidebar.sessionSelected(model.title)
                    onEntered: if (sidebar.selectedTitle !== model.title) parent.color = "#1a1a1a"
                    onExited: parent.color = sidebar.selectedTitle === model.title ? "#202020" : "transparent"
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "BACKGROUND AGENTS"
            color: theme.fgFaint
            font.family: theme.uiFont
            font.pixelSize: 10
            font.letterSpacing: 1.4
        }

        Item {
            Layout.fillWidth: true
            height: 38
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 2
                anchors.verticalCenter: parent.verticalCenter
                text: "↯"
                color: theme.fgDim
                font.pixelSize: 14
            }
            Column {
                anchors.left: parent.left
                anchors.leftMargin: 24
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1
                Text { text: "Neural networks"; color: theme.fg; font.family: theme.proseFont; font.pixelSize: 13 }
                Text { text: "Extraction"; color: theme.fgDim; font.family: theme.uiFont; font.pixelSize: 10 }
            }
            Text {
                anchors.right: parent.right
                anchors.rightMargin: 22
                anchors.verticalCenter: parent.verticalCenter
                text: "18s"
                color: theme.fgDim
                font.family: theme.uiFont
                font.pixelSize: 10
            }
            Rectangle {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 8
                height: 8
                color: "transparent"
                border.color: theme.fgDim
                border.width: 1
            }
        }

        Item {
            Layout.fillWidth: true
            height: 38
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 2
                anchors.verticalCenter: parent.verticalCenter
                text: "↯"
                color: theme.fgDim
                font.pixelSize: 14
            }
            Column {
                anchors.left: parent.left
                anchors.leftMargin: 24
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1
                Text { text: "Gradient descent"; color: theme.fg; font.family: theme.proseFont; font.pixelSize: 13 }
                Text { text: "delivering result"; color: theme.fgDim; font.family: theme.uiFont; font.pixelSize: 10 }
            }
            Text {
                anchors.right: parent.right
                anchors.rightMargin: 22
                anchors.verticalCenter: parent.verticalCenter
                text: "24s"
                color: theme.fgDim
                font.family: theme.uiFont
                font.pixelSize: 10
            }
            Rectangle {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 8
                height: 8
                color: "transparent"
                border.color: theme.fgDim
                border.width: 1
            }
        }

        Item { Layout.preferredHeight: 2 }

        Rectangle {
            Layout.fillWidth: true
            height: 34
            radius: 8
            color: "transparent"
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: "⚙  SETTINGS"
                color: theme.fgDim
                font.family: theme.uiFont
                font.pixelSize: 11
                font.letterSpacing: 1.2
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                onClicked: sidebar.openSettings()
                onEntered: parent.color = theme.panel
                onExited: parent.color = "transparent"
            }
        }
    }
}
