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
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);
    }
}

pub struct AppControllerRust {
    status: QString,
    version: QString,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("Phase 0 · Rust + Qt"),
            version: QString::from(env!("CARGO_PKG_VERSION")),
        }
    }
}

impl qobject::AppController {
    pub fn boot(mut self: Pin<&mut Self>) {
        self.as_mut().set_status(QString::from("Brainroom ready"));
    }
}
