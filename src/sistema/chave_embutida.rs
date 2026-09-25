//! kv.bin embutido na versão pessoal (compilada com `--features kv-embutido`).
//! Na versão normal não há chave nenhuma aqui.

#[cfg(feature = "kv-embutido")]
pub const KV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kv_embutido.bin"));
#[cfg(not(feature = "kv-embutido"))]
pub const KV: &[u8] = &[];

pub const EXISTE: bool = cfg!(feature = "kv-embutido");

/// Nome que aparece na janela.
pub const TITULO: &str = if EXISTE { "RE5 EDITOR PESSOAL GOLD" } else { "RE5 Save Editor 360+PC" };
