import QtQuick

Item {
    id: graph
    property var theme
    property real zoom: 1.0
    property real panX: 0
    property real panY: 0
    property var layoutNodes: []
    property var layoutEdges: []
    property var layoutClusters: []
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string selectedLabel: ""

    signal nodeActivated(string label)

    function mulberry32(a) {
        return function () {
            a |= 0;
            a = (a + 0x6D2B79F5) | 0;
            var t = Math.imul(a ^ (a >>> 15), 1 | a);
            t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
            return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
        };
    }

    function rebuildLayout(w, h) {
        if (w <= 20 || h <= 20)
            return;
        var palette = [
            "rgba(79,138,106,0.13)",
            "rgba(106,90,138,0.13)",
            "rgba(154,138,58,0.12)",
            "rgba(47,138,160,0.13)",
            "rgba(138,90,106,0.12)",
            "rgba(90,106,138,0.13)"
        ];
        var labels = [
            "loss surface", "compute gradient", "sample expectation", "unbiased estimator",
            "chain rule", "composition", "variance", "sample mean", "linearity",
            "expectation", "batch", "learning rate", "parameter space", "local minimum",
            "convexity", "derivative", "rate of change", "gradient descent", "minibatch",
            "convergence", "local geometry", "parameter update", "random error",
            "systematic bias", "estimate", "full gradient", "uniform sample"
        ];
        var rnd = mulberry32(1337);
        var base = Math.min(w, h);
        var ring = base * 0.31;
        var spread = base * 0.19;
        var clusters = [];
        var nodes = [];
        var edges = [];
        var labelIndex = 0;

        for (var c = 0; c < 6; c++) {
            var angle = -Math.PI / 2 + c * (Math.PI * 2 / 6);
            clusters.push({
                cx: w * 0.5 + Math.cos(angle) * ring,
                cy: h * 0.5 + Math.sin(angle) * ring,
                color: palette[c],
                glow: spread * 2.2
            });
        }

        for (var ci = 0; ci < clusters.length; ci++) {
            var cluster = clusters[ci];
            var count = 24 + Math.floor(rnd() * 10);
            var first = nodes.length;
            for (var n = 0; n < count; n++) {
                var theta = rnd() * Math.PI * 2;
                var radius = Math.pow(rnd(), 0.62) * spread;
                var label = "";
                if (labelIndex < labels.length && (n % 6 === 1 || n === 0)) {
                    label = labels[labelIndex++];
                }
                nodes.push({
                    x: cluster.cx + Math.cos(theta) * radius * 1.25,
                    y: cluster.cy + Math.sin(theta) * radius * 0.82,
                    r: 1.5 + rnd() * 1.2,
                    label: label
                });
            }
            for (var k = 1; k < count; k++) {
                var best = -1;
                var bestDist = 1e9;
                var a = nodes[first + k];
                for (var j = 0; j < k; j++) {
                    var b = nodes[first + j];
                    var d = (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y);
                    if (d < bestDist) {
                        bestDist = d;
                        best = j;
                    }
                }
                if (best >= 0 && bestDist < spread * spread * 0.42)
                    edges.push([first + k, first + best]);
                if (k > 2 && k % 5 === 0)
                    edges.push([first + k, first + Math.floor(rnd() * k)]);
            }
        }

        for (var ce = 1; ce < clusters.length; ce++) {
            edges.push([Math.floor(rnd() * 20), nodes.length - Math.floor(rnd() * 20) - 1]);
        }

        layoutClusters = clusters;
        layoutNodes = nodes;
        layoutEdges = edges;
        canvas.requestPaint();
    }

    function screenPosition(node) {
        return {
            x: width * 0.5 + panX + (node.x - width * 0.5) * zoom,
            y: height * 0.5 + panY + (node.y - height * 0.5) * zoom
        };
    }

    function nodeAt(mx, my) {
        for (var i = 0; i < layoutNodes.length; i++) {
            var p = screenPosition(layoutNodes[i]);
            var dx = mx - p.x;
            var dy = my - p.y;
            if (dx * dx + dy * dy < 11 * 11)
                return i;
        }
        return -1;
    }

    Canvas {
        id: canvas
        anchors.fill: parent
        renderStrategy: Canvas.Cooperative

        onPaint: {
            var ctx = getContext("2d");
            var w = width;
            var h = height;
            if (w <= 0 || h <= 0)
                return;
            var base = Math.min(w, h);

            ctx.clearRect(0, 0, w, h);
            var bg = ctx.createRadialGradient(w * 0.5, h * 0.5, 0, w * 0.5, h * 0.5, Math.max(w, h) * 0.78);
            bg.addColorStop(0, "#17151c");
            bg.addColorStop(0.52, "#0e0d12");
            bg.addColorStop(1, "#08080a");
            ctx.fillStyle = bg;
            ctx.fillRect(0, 0, w, h);

            ctx.save();
            ctx.translate(w * 0.5 + graph.panX, h * 0.5 + graph.panY);
            ctx.scale(graph.zoom, graph.zoom);
            ctx.translate(-w * 0.5, -h * 0.5);

            for (var gi = 0; gi < graph.layoutClusters.length; gi++) {
                var cl = graph.layoutClusters[gi];
                var glow = ctx.createRadialGradient(cl.cx, cl.cy, 0, cl.cx, cl.cy, cl.glow);
                glow.addColorStop(0, cl.color);
                glow.addColorStop(1, "rgba(0,0,0,0)");
                ctx.fillStyle = glow;
                ctx.beginPath();
                ctx.arc(cl.cx, cl.cy, cl.glow, 0, Math.PI * 2);
                ctx.fill();
            }

            ctx.lineCap = "round";
            for (var ei = 0; ei < graph.layoutEdges.length; ei++) {
                var e = graph.layoutEdges[ei];
                var a = graph.layoutNodes[e[0]];
                var b = graph.layoutNodes[e[1]];
                if (!a || !b)
                    continue;
                ctx.strokeStyle = "rgba(190,190,205,0.26)";
                ctx.lineWidth = 1;
                ctx.beginPath();
                ctx.moveTo(a.x, a.y);
                ctx.lineTo(b.x, b.y);
                ctx.stroke();
            }

            for (var ni = 0; ni < graph.layoutNodes.length; ni++) {
                var p = graph.layoutNodes[ni];
                var selected = ni === graph.selectedIndex;
                var hovered = ni === graph.hoveredIndex;
                var nodeGlow = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, selected || hovered ? 15 : 9);
                nodeGlow.addColorStop(0, selected ? "rgba(217,164,65,0.55)" : "rgba(230,230,240,0.16)");
                nodeGlow.addColorStop(1, "rgba(230,230,240,0)");
                ctx.fillStyle = nodeGlow;
                ctx.beginPath();
                ctx.arc(p.x, p.y, selected || hovered ? 15 : 9, 0, Math.PI * 2);
                ctx.fill();

                if (selected || hovered) {
                    ctx.strokeStyle = selected ? "#d9a441" : "rgba(230,230,240,0.6)";
                    ctx.lineWidth = 1.2 / graph.zoom;
                    ctx.beginPath();
                    ctx.arc(p.x, p.y, (selected ? 7 : 6) / graph.zoom + p.r, 0, Math.PI * 2);
                    ctx.stroke();
                }

                ctx.fillStyle = selected ? "#f0c66b" : "rgba(226,226,238,0.92)";
                ctx.beginPath();
                ctx.arc(p.x, p.y, p.r + (selected ? 0.7 : 0), 0, Math.PI * 2);
                ctx.fill();
            }

            ctx.font = "9px " + graph.theme.uiFont;
            ctx.fillStyle = "rgba(160,160,175,0.72)";
            for (var li = 0; li < graph.layoutNodes.length; li++) {
                var lp = graph.layoutNodes[li];
                if (lp.label)
                    ctx.fillText(lp.label, lp.x + 5, lp.y + 7);
            }

            ctx.restore();

            var vignette = ctx.createRadialGradient(w * 0.5, h * 0.5, base * 0.34, w * 0.5, h * 0.5, base * 0.82);
            vignette.addColorStop(0, "rgba(0,0,0,0)");
            vignette.addColorStop(1, "rgba(0,0,0,0.55)");
            ctx.fillStyle = vignette;
            ctx.fillRect(0, 0, w, h);
        }
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        hoverEnabled: true
        cursorShape: graph.hoveredIndex >= 0 ? Qt.PointingHandCursor : Qt.ArrowCursor

        property real startX: 0
        property real startY: 0
        property real startPanX: 0
        property real startPanY: 0
        property bool moved: false

        onPressed: {
            startX = mouseX;
            startY = mouseY;
            startPanX = graph.panX;
            startPanY = graph.panY;
            moved = false;
        }
        onPositionChanged: {
            if (pressed) {
                var dx = mouseX - startX;
                var dy = mouseY - startY;
                if (Math.abs(dx) + Math.abs(dy) > 4)
                    moved = true;
                if (moved) {
                    graph.panX = startPanX + dx;
                    graph.panY = startPanY + dy;
                }
            }
            var hit = graph.nodeAt(mouseX, mouseY);
            if (hit !== graph.hoveredIndex) {
                graph.hoveredIndex = hit;
                canvas.requestPaint();
            }
        }
        onReleased: {
            if (!moved) {
                var hit = graph.nodeAt(mouseX, mouseY);
                if (hit >= 0) {
                    graph.selectedIndex = hit;
                    graph.selectedLabel = graph.layoutNodes[hit].label;
                    graph.nodeActivated(graph.layoutNodes[hit].label);
                    canvas.requestPaint();
                }
            }
        }
        onExited: {
            graph.hoveredIndex = -1;
            canvas.requestPaint();
        }
        onWheel: function (wheel) {
            var factor = wheel.angleDelta.y > 0 ? 1.08 : 0.92;
            graph.zoom = Math.max(0.45, Math.min(3.5, graph.zoom * factor));
            canvas.requestPaint();
        }
    }

    onZoomChanged: canvas.requestPaint()
    onPanXChanged: canvas.requestPaint()
    onPanYChanged: canvas.requestPaint()
    onWidthChanged: rebuildLayout(width, height)
    onHeightChanged: rebuildLayout(width, height)
    Component.onCompleted: rebuildLayout(width, height)

    Rectangle {
        visible: graph.selectedIndex >= 0 && graph.selectedLabel !== ""
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.leftMargin: 24
        anchors.topMargin: 82
        width: labelText.implicitWidth + 24
        height: 32
        radius: 8
        color: "#1e1e1e"
        border.color: "#3a3a3a"
        border.width: 1
        Text {
            id: labelText
            anchors.centerIn: parent
            text: graph.selectedLabel
            color: graph.theme.fg
            font.family: graph.theme.uiFont
            font.pixelSize: 11
        }
    }
}
