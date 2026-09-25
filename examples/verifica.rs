//! Ferramenta de teste do núcleo (não entra no programa):
//!   verifica info <save>
//!   verifica edita <save> <saida> <mudancas.json> [kv.bin]
#[path = "../src/save.rs"]
mod save;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a[1].as_str() {
        "info" => println!("{}", save::Save::abre(Path::new(&a[2])).unwrap().info()),
        "edita" => {
            let mut s = save::Save::abre(Path::new(&a[2])).unwrap();
            let m: save::Mudancas = serde_json::from_str(&std::fs::read_to_string(&a[4]).unwrap()).unwrap();
            s.aplica(&m).unwrap();
            let kv = a.get(5).map(Path::new);
            let (bytes, console) = s.monta(kv).unwrap();
            std::fs::write(&a[3], bytes).unwrap();
            println!("assinado={:?} confere={}", console, save::Save::confere(Path::new(&a[3]), console.is_some()));
        }
        _ => unreachable!(),
    }
}
