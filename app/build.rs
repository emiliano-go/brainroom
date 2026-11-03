use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_files = vec![std::path::PathBuf::from("qml/Main.qml")];

    CxxQtBuilder::new_qml_module(QmlModule::new("Brainroom").qml_files(qml_files))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/bridge.rs"])
        .build();
}
