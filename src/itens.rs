//! Tabela de itens do jogo (re5_items.json) e as classes de item.
//!
//! O byte alto do ID é a classe: 01 arma, 02 munição, 03 cura, 04 tesouro,
//! 05 chave, 06 colete, 07 arquivo.

use std::collections::BTreeMap;

const ITENS_JSON: &str = include_str!("re5_items.json");

/// Nome de cada classe, na ordem do byte alto do ID.
pub const CLASSES: [&str; 8] = ["Vazio", "Arma", "Munição", "Cura / acessório", "Tesouro", "Chave", "Colete", "Arquivo"];

pub fn classe_de(id: u32) -> usize {
    (id >> 8) as usize
}

pub struct Itens {
    nomes: BTreeMap<u32, String>,
}

impl Itens {
    pub fn carrega() -> Self {
        #[derive(serde::Deserialize)]
        struct Bruto {
            name: String,
        }
        let bruto: BTreeMap<String, Bruto> = serde_json::from_str(ITENS_JSON).expect("re5_items.json inválido");
        let nomes = bruto
            .into_iter()
            .filter_map(|(k, v)| u32::from_str_radix(k.trim_start_matches("0x"), 16).ok().map(|id| (id, v.name)))
            .filter(|(id, _)| *id != 0)
            .collect();
        Self { nomes }
    }

    /// Nome para mostrar; itens fora da tabela aparecem pela classe e o ID.
    pub fn nome(&self, id: u32) -> String {
        if id == 0 {
            return "Vazio".into();
        }
        self.nomes.get(&id).cloned().unwrap_or_else(|| {
            format!("{} 0x{id:04x}", CLASSES.get(classe_de(id)).copied().unwrap_or("Item"))
        })
    }

    pub fn nome_classe(id: u32) -> &'static str {
        if id == 0 {
            return "";
        }
        CLASSES.get(classe_de(id)).copied().unwrap_or("?")
    }

    /// IDs de uma classe, em ordem. Um ID desconhecido da mesma classe entra
    /// no começo, para não sumir da lista ao editar o slot.
    pub fn da_classe(&self, classe: usize, atual: u32) -> Vec<u32> {
        if classe == 0 {
            return vec![0];
        }
        let mut ids: Vec<u32> = self.nomes.keys().copied().filter(|&id| classe_de(id) == classe).collect();
        if atual != 0 && classe_de(atual) == classe && !ids.contains(&atual) {
            ids.insert(0, atual);
        }
        if ids.is_empty() {
            ids.push(0);
        }
        ids
    }

    /// Texto da lista de itens: nome e o ID em hexadecimal.
    pub fn rotulo(&self, id: u32) -> String {
        if id == 0 {
            "Vazio".into()
        } else {
            format!("{}  (0x{id:04x})", self.nome(id))
        }
    }
}
