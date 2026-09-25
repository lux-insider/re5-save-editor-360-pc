//! Onde ficam o keyvault, os backups e por onde os diálogos começam.
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

pub fn pasta_do_exe() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| ".".into())
}

/// Keyvault do console: `console/kv.bin` ao lado do programa (portátil);
/// no Linux, também o de ~/horizon_final (só leitura).
pub fn keyvault() -> Option<PathBuf> {
    [pasta_do_exe().join("console").join("kv.bin"), home().join("horizon_final").join("console").join("kv.bin")]
        .into_iter()
        .find(|p| p.is_file())
}

/// Backups ao lado do programa; se a pasta não for gravável, na pasta de
/// dados do usuário.
pub fn pasta_backups() -> Result<PathBuf, String> {
    let local = pasta_do_exe().join("backups");
    if std::fs::create_dir_all(&local).is_ok() && gravavel(&local) {
        return Ok(local);
    }
    let dados = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("re5-save-editor-360-pc")
        .join("backups");
    std::fs::create_dir_all(&dados).map_err(|e| format!("backup: {e}"))?;
    Ok(dados)
}

fn gravavel(p: &Path) -> bool {
    let t = p.join(".teste-escrita");
    let ok = std::fs::write(&t, b"").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

/// Onde os diálogos começam: a pasta do save aberto ou um lugar provável.
pub fn pasta_inicial(atual: Option<&Path>) -> PathBuf {
    if let Some(p) = atual.and_then(Path::parent) {
        return p.into();
    }
    let h = home();
    let candidatos = [
        h.join("Documentos").join("434307D4").join("00000001"),
        h.join("Documents").join("434307D4").join("00000001"),
        PathBuf::from(r"C:\Program Files (x86)\Steam\userdata"),
    ];
    candidatos.into_iter().find(|p| p.is_dir()).unwrap_or(h)
}
