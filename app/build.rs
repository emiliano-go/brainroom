use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let mut qml_files: Vec<std::path::PathBuf> = std::fs::read_dir("qml")
        .expect("qml directory")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|ext| ext == "qml").unwrap_or(false))
        .collect();
    qml_files.sort();

    CxxQtBuilder::new_qml_module(QmlModule::new("Brainroom").qml_files(qml_files))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qrc("resources.qrc")
        .files(["src/bridge.rs"])
        .build();
}
