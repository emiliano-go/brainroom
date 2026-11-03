import QtQuick
import QtQuick.Controls

Rectangle {
    id: card

    property var theme
    property string question: ""
    property var options: []
    property int selectedIndex: 2
    property real confidence: 50
    property string thinking: ""

    signal submitted()

    implicitHeight: contentColumn.implicitHeight + 24
    height: implicitHeight

    color: "#151515"
    border.color: "#2e2e2e"
    border.width: 1
    radius: 12

    Column {
        id: contentColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 12
        spacing: 8

        Text {
            text: "PREDICTION"
            color: card.theme.fgFaint
            font.family: card.theme.uiFont
            font.pixelSize: 10
            font.letterSpacing: 1.5
        }

        Text {
            width: parent.width
            text: card.question
            color: card.theme.fg
            font.family: card.theme.proseFont
            font.pixelSize: 13
            textFormat: Text.RichText
            lineHeight: 1.35
            wrapMode: Text.WordWrap
        }

        Repeater {
            model: card.options
            delegate: Rectangle {
                width: parent.width
                height: optionLabel.implicitHeight + 14
                radius: 8
                color: card.selectedIndex === index ? "#202020" : "transparent"
                border.color: card.selectedIndex === index ? "#555555" : "#3a3a3a"
                border.width: 1

                Rectangle {
                    id: radio
                    anchors.left: parent.left
                    anchors.leftMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    width: 16
                    height: 16
                    radius: 8
                    color: "transparent"
                    border.color: card.selectedIndex === index ? card.theme.fg : "#555555"
                    border.width: 1
                    Rectangle {
                        visible: card.selectedIndex === index
                        anchors.centerIn: parent
                        width: 7
                        height: 7
                        radius: 4
                        color: card.theme.fg
                    }
                }

                Text {
                    id: optionLabel
                    anchors.left: radio.right
                    anchors.leftMargin: 12
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData
                    color: card.theme.fg
                    font.family: card.theme.proseFont
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                    textFormat: Text.RichText
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: card.selectedIndex = index
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 34
            radius: 8
            color: card.selectedIndex === -1 ? "#202020" : "transparent"
            border.color: "#3a3a3a"
            border.width: 1
            Rectangle {
                anchors.left: parent.left
                anchors.leftMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                width: 16
                height: 16
                radius: 8
                color: "transparent"
                border.color: "#555555"
                border.width: 1
            }
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 42
                anchors.verticalCenter: parent.verticalCenter
                text: "I don't know"
                color: card.theme.fgDim
                font.family: card.theme.proseFont
                font.pixelSize: 12
            }
            MouseArea {
                anchors.fill: parent
                onClicked: card.selectedIndex = -1
            }
        }

        Item {
            width: parent.width
            height: 22

            Text {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "CONFIDENCE"
                color: card.theme.fgFaint
                font.family: card.theme.uiFont
                font.pixelSize: 10
                font.letterSpacing: 1.3
            }

            Item {
                id: slider
                anchors.left: parent.left
                anchors.leftMargin: 92
                anchors.right: confidenceText.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                height: 18

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: 2
                    color: "#3a3a3a"
                }
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width * (card.confidence / 100)
                    height: 2
                    color: card.theme.gold
                }
                Repeater {
                    model: 5
                    delegate: Rectangle {
                        x: index * (slider.width / 4) - 0.5
                        anchors.verticalCenter: parent.verticalCenter
                        width: 1
                        height: 8
                        color: "#464646"
                    }
                }
                Rectangle {
                    x: parent.width * (card.confidence / 100) - 7
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    radius: 7
                    color: card.theme.gold
                }
                MouseArea {
                    anchors.fill: parent
                    onPositionChanged: {
                        if (pressed)
                            card.confidence = Math.max(0, Math.min(100, Math.round(mouseX / width * 100)));
                    }
                    onPressed: card.confidence = Math.max(0, Math.min(100, Math.round(mouseX / width * 100)))
                }
            }

            Text {
                id: confidenceText
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: Math.round(card.confidence) + "%"
                color: card.theme.fg
                font.family: card.theme.uiFont
                font.pixelSize: 12
            }
        }

        Rectangle {
            width: parent.width
            height: 64
            radius: 8
            color: "#121212"
            border.color: "#2e2e2e"
            border.width: 1

            Column {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 3
                Text {
                    text: "YOUR THINKING"
                    color: card.theme.fgFaint
                    font.family: card.theme.uiFont
                    font.pixelSize: 10
                    font.letterSpacing: 1.3
                }
                TextArea {
                    width: parent.width
                    height: parent.height - 14
                    text: card.thinking
                    placeholderText: "Why this answer? Whatever comes to mind."
                    color: card.theme.fg
                    placeholderTextColor: card.theme.fgFaint
                    selectionColor: card.theme.border
                    font.family: card.theme.proseFont
                    font.pixelSize: 12
                    wrapMode: TextEdit.Wrap
                    background: null
                    padding: 0
                }
            }
        }

        Item {
            width: parent.width
            height: 30

            Text {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "↵ submits   ⇥ newline"
                color: card.theme.fgDim
                font.family: card.theme.uiFont
                font.pixelSize: 10
            }

            Rectangle {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 146
                height: 30
                radius: 8
                color: "#202020"
                border.color: "#3a3a3a"
                border.width: 1
                Text {
                    anchors.centerIn: parent
                    text: "Submit answer"
                    color: card.theme.fg
                    font.family: card.theme.proseFont
                    font.pixelSize: 12
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: card.submitted()
                }
            }
        }
    }
}
