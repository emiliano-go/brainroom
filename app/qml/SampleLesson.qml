import QtQuick

Column {
    id: sample
    property var theme
    width: parent ? parent.width : 820
    spacing: 10

    Text {
        width: parent.width
        text: "Freeze the current parameters <i>x</i>. The full gradient is"
        color: sample.theme.fg
        font.family: sample.theme.proseFont
        font.pixelSize: 15
        lineHeight: 1.4
        wrapMode: Text.WordWrap
        textFormat: Text.RichText
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: "g = ∇f(x) = <sup>1</sup>⁄<sub>N</sub> Σ<sub>i=1</sub><sup>N</sup> ∇ℓ<sub>i</sub>(x)."
        color: sample.theme.fg
        font.family: sample.theme.mathFont
        font.pixelSize: 20
        textFormat: Text.RichText
    }

    Text {
        width: parent.width
        text: "Choose one example <i>l</i> uniformly and use its gradient as the estimate, <i>ĝ</i> = ∇ℓ<sub>l</sub>(x). Each example has probability 1/N, so"
        color: sample.theme.fg
        font.family: sample.theme.proseFont
        font.pixelSize: 15
        lineHeight: 1.4
        wrapMode: Text.WordWrap
        textFormat: Text.RichText
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: "E[ĝ] = Σ<sub>i=1</sub><sup>N</sup> <sup>1</sup>⁄<sub>N</sub> ∇ℓ<sub>i</sub>(x) = g."
        color: sample.theme.fg
        font.family: sample.theme.mathFont
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
            color: sample.theme.fg
            font.family: sample.theme.proseFont
            font.pixelSize: 14
            lineHeight: 1.35
            wrapMode: Text.WordWrap
            textFormat: Text.RichText
        }
    }

    Text {
        width: parent.width
        text: "For example, suppose two examples have derivatives −2 and 6 at the current parameter. The full derivative is 2. Uniform sampling returns either −2 or 6, but"
        color: sample.theme.fg
        font.family: sample.theme.proseFont
        font.pixelSize: 15
        lineHeight: 1.4
        wrapMode: Text.WordWrap
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: "E[ĝ] = <sup>1</sup>⁄<sub>2</sub>(−2) + <sup>1</sup>⁄<sub>2</sub>(6) = 2."
        color: sample.theme.fg
        font.family: sample.theme.mathFont
        font.pixelSize: 20
    }

    Text {
        width: parent.width
        text: "The individual errors are −4 and +4; they cancel <b>on average</b>. One estimate can even point opposite to the full gradient without making the estimator biased. Random error and systematic bias are different things."
        color: sample.theme.fg
        font.family: sample.theme.proseFont
        font.pixelSize: 15
        lineHeight: 1.4
        wrapMode: Text.WordWrap
        textFormat: Text.RichText
    }

    PredictionCard {
        width: parent.width
        theme: sample.theme
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
