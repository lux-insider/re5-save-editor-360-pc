// RE5 Save Editor 360+PC — interface em Slint.
//
//   save.rs      backend: lê e grava o save (Xbox 360 e PC)
//   itens.rs     tabela de itens
//   sistema/     pastas, keyvault e diálogos de arquivo
//   interface/   controlador: liga a interface (ui/*.slint) ao backend
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod interface;
mod itens;
mod save;
mod sistema;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = JanelaPrincipal::new()?;
    let inicial = std::env::args().skip(1).find(|a| !a.starts_with('-')).map(std::path::PathBuf::from).filter(|p| p.is_file());
    interface::iniciar(&ui, inicial);
    ui.run()
}
