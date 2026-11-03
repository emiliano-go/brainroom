use core::pin::Pin;

use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status, cxx_name = "status")]
        #[qproperty(QString, version, cxx_name = "version")]
        #[qproperty(QString, initial_view, cxx_name = "initialView")]
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath")]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);
    }
}

pub struct AppControllerRust {
    status: QString,
    version: QString,
    initial_view: QString,
    screenshot_path: QString,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let initial_view = if args.iter().any(|arg| arg == "--home") {
            "home"
        } else {
            "session"
        };
        let screenshot_path = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--screenshot="))
            .unwrap_or("");
        Self {
            status: QString::from("Phase 0 · Rust + Qt"),
            version: QString::from(env!("CARGO_PKG_VERSION")),
            initial_view: QString::from(initial_view),
            screenshot_path: QString::from(screenshot_path),
        }
    }
}

impl qobject::AppController {
    pub fn boot(mut self: Pin<&mut Self>) {
        self.as_mut().set_status(QString::from("Brainroom ready"));
    }
}
