import QtQuick

Item {
    id: minimap
    property var theme
    opacity: 0.42

    readonly property var nodes: [
        { x: 16, y: 22, w: 92, h: 18, label: "loss surface" },
        { x: 142, y: 8, w: 118, h: 18, label: "compute gradient" },
        { x: 142, y: 42, w: 118, h: 18, label: "sample expectation" },
        { x: 142, y: 76, w: 118, h: 18, label: "unbiased estimator" },
        { x: 16, y: 108, w: 92, h: 18, label: "minibatch" },
        { x: 142, y: 110, w: 118, h: 18, label: "variance" },
        { x: 16, y: 142, w: 92, h: 18, label: "rate of change" },
        { x: 142, y: 144, w: 118, h: 18, label: "gradient descent" },
        { x: 142, y: 178, w: 118, h: 18, label: "parameter update", highlight: true },
        { x: 16, y: 210, w: 92, h: 18, label: "local geometry" },
        { x: 142, y: 212, w: 118, h: 18, label: "convergence" }
    ]

    readonly property var edges: [
        [0, 2], [1, 2], [2, 3], [2, 4], [2, 5], [3, 6], [3, 7], [7, 8], [4, 9], [8, 10]
    ]

    Canvas {
        anchors.fill: parent
        onPaint: {
            var ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            ctx.strokeStyle = "rgba(180,180,198,0.38)";
            ctx.lineWidth = 1;
            for (var i = 0; i < minimap.edges.length; i++) {
                var e = minimap.edges[i];
                var a = minimap.nodes[e[0]];
                var b = minimap.nodes[e[1]];
                ctx.beginPath();
                ctx.moveTo(a.x + a.w / 2, a.y + a.h);
                ctx.lineTo(b.x + b.w / 2, b.y);
                ctx.stroke();
            }
        }
    }

    Repeater {
        model: minimap.nodes
        delegate: Rectangle {
            x: modelData.x
            y: modelData.y
            width: modelData.w
            height: modelData.h
            radius: 3
            color: modelData.highlight ? "rgba(217,164,65,0.32)" : "rgba(24,24,28,0.8)"
            border.color: modelData.highlight ? minimap.theme.gold : "rgba(120,120,136,0.45)"
            border.width: 1
            Text {
                anchors.centerIn: parent
                text: modelData.label
                color: modelData.highlight ? minimap.theme.gold : "#9a9aa8"
                font.family: minimap.theme.uiFont
                font.pixelSize: 6
            }
        }
    }
}
