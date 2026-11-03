import QtQuick

Item {
    id: theme
    visible: false
    width: 0
    height: 0

    FontLoader {
        id: cmu
        source: "qrc:/assets/cmunbx.ttf"
    }

    readonly property color bg: "#141414"
    readonly property color panel: "#1c1c1c"
    readonly property color panelRaised: "#202020"
    readonly property color panelHover: "#232323"
    readonly property color border: "#434343"
    readonly property color borderSoft: "#2b2b2b"
    readonly property color fg: "#e6e6e6"
    readonly property color fgDim: "#8d8d8d"
    readonly property color fgFaint: "#5f5f5f"
    readonly property color gold: "#d9a441"
    readonly property color red: "#c76a6a"
    readonly property color green: "#61c554"
    readonly property color yellow: "#f4bf4f"

    readonly property string uiFont: "CaskaydiaMono Nerd Font"
    readonly property string proseFont: "Adwaita Sans"
    readonly property string mathFont: "Crimson"
    readonly property string titleFont: cmu.name.length > 0 ? cmu.name : "CMU Serif"

    readonly property int radius: 8
    readonly property int radiusLg: 12
}
