//! Diálogos nativos: abrir, salvar como e confirmar.
//!
//! No Linux usa o GTK direto, na thread da janela: o rfd abre o GTK numa
//! thread própria e, com o GTK já iniciado pelo tao, as duas ficavam
//! esperando uma pela outra (a janela travava ao clicar em "Escolher
//! arquivo"). No Windows o rfd usa os diálogos do próprio sistema.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
pub struct Dialogos {
    pai: gtk::ApplicationWindow,
}

#[cfg(target_os = "linux")]
impl Dialogos {
    pub fn novo(janela: &tao::window::Window) -> Self {
        use tao::platform::unix::WindowExtUnix;
        Self { pai: janela.gtk_window().clone() }
    }

    fn escolhe(&self, titulo: &str, pasta: &Path, salvar: Option<&str>) -> Option<PathBuf> {
        use gtk::prelude::*;
        let (acao, botao) = if salvar.is_some() {
            (gtk::FileChooserAction::Save, "_Salvar")
        } else {
            (gtk::FileChooserAction::Open, "_Abrir")
        };
        let d = gtk::FileChooserDialog::with_buttons(
            Some(titulo),
            Some(&self.pai),
            acao,
            &[("_Cancelar", gtk::ResponseType::Cancel), (botao, gtk::ResponseType::Accept)],
        );
        d.set_default_response(gtk::ResponseType::Accept);
        d.set_current_folder(pasta);
        if let Some(nome) = salvar {
            d.set_current_name(nome);
            d.set_do_overwrite_confirmation(true);
        }
        let r = d.run();
        let arquivo = d.filename();
        // SAFETY: o diálogo é nosso e não é usado depois daqui
        unsafe { d.destroy() };
        (r == gtk::ResponseType::Accept).then_some(arquivo).flatten()
    }

    pub fn abrir(&self, titulo: &str, pasta: &Path) -> Option<PathBuf> {
        self.escolhe(titulo, pasta, None)
    }

    pub fn salvar(&self, titulo: &str, pasta: &Path, nome: &str) -> Option<PathBuf> {
        self.escolhe(titulo, pasta, Some(nome))
    }

    pub fn confirmar(&self, titulo: &str, texto: &str) -> bool {
        use gtk::prelude::*;
        let d = gtk::MessageDialog::new(
            Some(&self.pai),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Warning,
            gtk::ButtonsType::YesNo,
            texto,
        );
        d.set_title(titulo);
        let r = d.run();
        // SAFETY: idem
        unsafe { d.destroy() };
        r == gtk::ResponseType::Yes
    }
}

#[cfg(not(target_os = "linux"))]
pub struct Dialogos {
    pai: rfd::FileDialog,
}

#[cfg(not(target_os = "linux"))]
impl Dialogos {
    pub fn novo(janela: &tao::window::Window) -> Self {
        Self { pai: rfd::FileDialog::new().set_parent(janela) }
    }

    pub fn abrir(&self, titulo: &str, pasta: &Path) -> Option<PathBuf> {
        self.pai.clone().set_title(titulo).set_directory(pasta).pick_file()
    }

    pub fn salvar(&self, titulo: &str, pasta: &Path, nome: &str) -> Option<PathBuf> {
        self.pai.clone().set_title(titulo).set_directory(pasta).set_file_name(nome).save_file()
    }

    pub fn confirmar(&self, titulo: &str, texto: &str) -> bool {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(titulo)
            .set_description(texto)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }
}

/// Mostra um erro que impede o programa de abrir e encerra.
pub fn erro_fatal(texto: &str) -> ! {
    #[cfg(not(target_os = "linux"))]
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("RE5 Save Editor 360+PC")
        .set_description(texto)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    eprintln!("{texto}");
    std::process::exit(1)
}
