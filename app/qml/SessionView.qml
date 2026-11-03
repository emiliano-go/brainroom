import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Rectangle {
    id: session

    property var theme
    property string lessonTitle: "Gradient descent"

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

            Text {
                width: parent.width
                text: "Freeze the current parameters <i>x</i>. The full gradient is"
                color: session.theme.fg
                font.family: session.theme.proseFont
                font.pixelSize: 15
                lineHeight: 1.4
                wrapMode: Text.WordWrap
                textFormat: Text.RichText
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "g = ∇f(x) = <sup>1</sup>⁄<sub>N</sub> Σ<sub>i=1</sub><sup>N</sup> ∇ℓ<sub>i</sub>(x)."
                color: session.theme.fg
                font.family: session.theme.mathFont
                font.pixelSize: 20
                textFormat: Text.RichText
            }

            Text {
                width: parent.width
                text: "Choose one example <i>l</i> uniformly and use its gradient as the estimate, <i>ĝ</i> = ∇ℓ<sub>l</sub>(x). Each example has probability 1/N, so"
                color: session.theme.fg
                font.family: session.theme.proseFont
                font.pixelSize: 15
                lineHeight: 1.4
                wrapMode: Text.WordWrap
                textFormat: Text.RichText
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "E[ĝ] = Σ<sub>i=1</sub><sup>N</sup> <sup>1</sup>⁄<sub>N</sub> ∇ℓ<sub>i</sub>(x) = g."
                color: session.theme.fg
                font.family: session.theme.mathFont
                font.pixelSize: 20
                textFormat: Text.RichText
            }

            Rectangle {
                width: parent.width
                height: calloutText.implicitHeight + 24
                radius: 4
                color: "transparent"
                border.color: "#4a4a4a"
                border.width: 1

                Text {
                    id: calloutText
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    text: "<b>Unbiased:</b> the estimator's expectation equals the quantity we want to estimate. It does <b>not</b> mean each individual estimate is accurate."
                    color: session.theme.fg
                    font.family: session.theme.proseFont
                    font.pixelSize: 14
                    lineHeight: 1.35
                    wrapMode: Text.WordWrap
                    textFormat: Text.RichText
                }
            }

            Text {
                width: parent.width
                text: "For example, suppose two examples have derivatives −2 and 6 at the current parameter. The full derivative is 2. Uniform sampling returns either −2 or 6, but"
                color: session.theme.fg
                font.family: session.theme.proseFont
                font.pixelSize: 15
                lineHeight: 1.4
                wrapMode: Text.WordWrap
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "E[ĝ] = <sup>1</sup>⁄<sub>2</sub>(−2) + <sup>1</sup>⁄<sub>2</sub>(6) = 2."
                color: session.theme.fg
                font.family: session.theme.mathFont
                font.pixelSize: 20
            }

            Text {
                width: parent.width
                text: "The individual errors are −4 and +4; they cancel <b>on average</b>. One estimate can even point opposite to the full gradient without making the estimator biased. Random error and systematic bias are different things."
                color: session.theme.fg
                font.family: session.theme.proseFont
                font.pixelSize: 15
                lineHeight: 1.4
                wrapMode: Text.WordWrap
                textFormat: Text.RichText
            }

            PredictionCard {
                width: parent.width
                theme: session.theme
                question: "For f(x) = 1/N Σ<sub>i</sub> ℓ<sub>i</sub>(x), what is ∇f, and what is the nature of a minibatch estimate of it?"
                options: [
                    "There's no expectation here — the training examples are fixed, not random.",
                    "A minibatch estimate is systematically biased, pointing in a subtly wrong direction.",
                    "It's 1/N Σ<sub>i</sub> ∇ℓ<sub>i</sub>, and a minibatch average is an unbiased estimate whose variance shrinks as the batch grows.",
                    "It's a fixed quantity that equals the gradient exactly for every batch."
                ]
                selectedIndex: 2
                confidence: 50
            }

            Item { width: 1; height: 24 }
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
