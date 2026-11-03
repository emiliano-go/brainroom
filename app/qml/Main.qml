import QtQuick
import QtQuick.Controls
import QtQuick.Window
import Brainroom 1.0

ApplicationWindow {
    id: root
    visible: true
    width: 1440
    height: 900
    minimumWidth: 1000
    minimumHeight: 700
    title: "Brainroom"
    color: theme.bg
    flags: Qt.FramelessWindowHint

    property string view: "session"
    property string sessionTitle: "Gradient descent"
    property bool settingsOpen: false
    property string screenshotPath: ""

    AppController { id: app }

    Theme { id: theme }

    Rectangle {
        anchors.fill: parent
        color: theme.bg

        Sidebar {
            id: sidebar
            width: 280
            height: parent.height
            visible: root.view === "session"
            theme: theme
            selectedTitle: root.sessionTitle
            onNewTeaching: {
                root.sessionTitle = "Gradient descent";
                root.view = "session";
            }
            onSessionSelected: function (title) {
                root.sessionTitle = title;
                root.view = "session";
            }
            onOpenSettings: root.settingsOpen = true
            onGoHome: root.view = "home"
            onCloseWindow: root.close()
            onMinimizeWindow: root.showMinimized()
            onMaximizeWindow: root.visibility === Window.Maximized ? root.showNormal() : root.showMaximized()
        }

        HomeView {
            anchors.left: root.view === "session" ? sidebar.right : parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            visible: root.view === "home"
            theme: theme
            onSubmitPrompt: function (text) {
                app.send(text);
                root.sessionTitle = text.length > 0 ? text : "New session";
                root.view = "session";
            }
            onOpenSettings: root.settingsOpen = true
            onToggleSidebar: root.view = "session"
            onCloseWindow: root.close()
            onMinimizeWindow: root.showMinimized()
            onMaximizeWindow: root.visibility === Window.Maximized ? root.showNormal() : root.showMaximized()
        }

        SessionView {
            anchors.left: sidebar.right
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            visible: root.view === "session"
            theme: theme
            lessonTitle: root.sessionTitle
            streamText: app.streamText
            errorText: app.errorText
            busy: app.busy
            onOpenSettings: root.settingsOpen = true
            onToggleSidebar: root.view = "home"
        }

        SettingsDialog {
            anchors.fill: parent
            visible: root.settingsOpen
            theme: theme
            onClosed: root.settingsOpen = false
        }
    }

    Timer {
        id: fallbackTimer
        interval: 5000
        repeat: false
        running: screenshotPath !== ""
        onTriggered: Qt.quit()
    }

    Timer {
        interval: 100
        repeat: true
        running: true
        onTriggered: app.poll()
    }

    Timer {
        id: shotTimer
        interval: 1100
        repeat: false
        running: false
        onTriggered: {
            root.contentItem.grabToImage(function (result) {
                if (root.screenshotPath !== "")
                    result.saveToFile(root.screenshotPath);
                Qt.quit();
            }, Qt.size(root.width, root.height));
        }
    }

    Component.onCompleted: {
        view = app.initialView;
        screenshotPath = app.screenshotPath;
        if (screenshotPath !== "")
            shotTimer.start();
        if (app.initialPrompt !== "") {
            view = "session";
            app.send(app.initialPrompt);
        }
    }
}
