//! Diálogos nativos de abrir e salvar (rfd: GTK no Linux, o do Windows no
//! Windows). Confirmações e avisos são desenhados pela própria interface.
use std::path::{Path, PathBuf};

pub fn abrir(titulo: &str, pasta: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(titulo).set_directory(pasta).pick_file()
}

pub fn salvar(titulo: &str, pasta: &Path, nome: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(titulo).set_directory(pasta).set_file_name(nome).save_file()
}
