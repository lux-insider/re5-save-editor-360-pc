//! Catálogo dos 407 registros de item do RE5 (src/dados/itens.json).
//!
//! Os dados vêm da engenharia reversa da ISO e das DLCs: tabela
//! ITEM_INFO_STRUCT do jogo, nomes do jogo e das DLCs, ícones originais e a
//! classificação de cada registro. O byte alto do ID é a classe: 01 arma,
//! 02 munição, 03 cura, 04 tesouro, 05 chave, 06 colete, 07 arquivo.
//!
//! Regra de gravação: só entra nas listas de escolha o que o editor já
//! gravava (testado no console) e tem nome no jogo ou numa DLC. O resto é
//! só para ver (abas Itens Extras e História).

use serde::Deserialize;
use std::collections::BTreeMap;

const ITENS_JSON: &str = include_str!("dados/itens.json");

/// Nome de cada classe, na ordem do byte alto do ID.
pub const CLASSES: [&str; 8] = ["Vazio", "Arma", "Munição", "Cura / acessório", "Tesouro", "Chave", "Colete", "Arquivo"];

pub fn classe_de(id: u32) -> usize {
    (id >> 8) as usize
}

/// Um registro da tabela de itens do jogo.
#[derive(Clone, Debug, Deserialize)]
pub struct Item {
    pub id: u32,
    pub categoria: String,
    pub secao: String,
    /// Nome mostrado no editor: "português (inglês oficial)".
    pub nome: Option<String>,
    /// Nome oficial em inglês (jogo ou DLC).
    pub oficial: Option<String>,
    /// Nome em português (tradução do projeto).
    pub pt: Option<String>,
    pub interno: String,
    #[serde(rename = "enum")]
    pub enumerador: Option<String>,
    pub dlc: Option<String>,
    pub descricao: Option<String>,
    /// Posição no atlas de ícones (ui/imagens/icones.png); -1 = sem ícone.
    pub icone: i32,
    /// "original" (ícone do próprio item), "mesmo_objeto" (ícone de outro ID
    /// que é o mesmo objeto) ou "silhueta" (silhueta da categoria, da loja do
    /// jogo); nenhum quando o jogo não tem imagem.
    pub icone_tipo: Option<String>,
    pub gravavel: bool,
    pub legado: bool,
    /// O jogo marca como item dos slots do personagem (mbSlot).
    pub slot: bool,
    pub coletavel: bool,
    pub colocacoes: u32,
    pub obs: Option<String>,
}

/// Onde o item vai ser gravado.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Destino {
    /// Os 9 slots do Chris ou da Sheva.
    Personagem,
    /// Os 84 espaços do inventário (baú).
    Inventario,
}

/// Grupos da aba Itens Extras: (rótulo, categorias).
pub const GRUPOS_EXTRAS: [(&str, &[&str]); 5] = [
    ("Armas de inimigos", &["arma_inimigo"]),
    ("Objetos de fase", &["objeto_fase"]),
    ("Itens das DLCs", &["item_dlc", "arma_dlc"]),
    ("Não testados", &["arma", "granada_ovo", "municao", "cura", "outros"]),
    ("Internos", &["interno", "interno_dummy"]),
];

pub struct Itens {
    por_id: BTreeMap<u32, Item>,
    pub colunas_atlas: i32,
    pub celula_atlas: i32,
}

impl Itens {
    pub fn carrega() -> Self {
        #[derive(Deserialize)]
        struct Arquivo {
            colunas_atlas: i32,
            celula: i32,
            itens: Vec<Item>,
        }
        let a: Arquivo = serde_json::from_str(ITENS_JSON).expect("src/dados/itens.json inválido");
        Self {
            por_id: a.itens.into_iter().map(|i| (i.id, i)).collect(),
            colunas_atlas: a.colunas_atlas,
            celula_atlas: a.celula,
        }
    }

    pub fn item(&self, id: u32) -> Option<&Item> {
        self.por_id.get(&id)
    }

    #[cfg(test)]
    pub fn todos(&self) -> impl Iterator<Item = &Item> {
        self.por_id.values()
    }

    /// Nome para mostrar; itens sem nome aparecem pela classe e o ID.
    pub fn nome(&self, id: u32) -> String {
        if id == 0 {
            return "Vazio".into();
        }
        self.item(id).and_then(|i| i.nome.clone()).unwrap_or_else(|| {
            format!("{} 0x{id:04x}", CLASSES.get(classe_de(id)).copied().unwrap_or("Item"))
        })
    }

    pub fn nome_classe(id: u32) -> &'static str {
        if id == 0 {
            return "";
        }
        CLASSES.get(classe_de(id)).copied().unwrap_or("?")
    }

    pub fn icone(&self, id: u32) -> i32 {
        self.item(id).map(|i| i.icone).unwrap_or(-1)
    }

    /// IDs que podem ser escolhidos numa classe. Só os graváveis; nos slots
    /// do Chris e da Sheva, só os que o jogo marca como item de slot. O ID
    /// atual entra sempre no começo, para não sumir da lista ao editar.
    pub fn opcoes(&self, classe: usize, atual: u32, destino: Destino) -> Vec<u32> {
        if classe == 0 {
            return vec![0];
        }
        let mut ids: Vec<u32> = self
            .por_id
            .values()
            .filter(|i| classe_de(i.id) == classe && i.gravavel)
            .filter(|i| destino == Destino::Inventario || i.slot)
            .map(|i| i.id)
            .collect();
        if atual != 0 && classe_de(atual) == classe && !ids.contains(&atual) {
            ids.insert(0, atual);
        }
        ids
    }

    /// Classes que têm alguma opção para o destino (a "Vazio" sempre).
    pub fn classes_para(&self, destino: Destino) -> Vec<usize> {
        (0..CLASSES.len()).filter(|&c| c == 0 || !self.opcoes(c, 0, destino).is_empty()).collect()
    }

    /// Registros de um grupo da aba Itens Extras.
    pub fn extras(&self, grupo: usize) -> Vec<&Item> {
        let Some((_, categorias)) = GRUPOS_EXTRAS.get(grupo) else { return Vec::new() };
        self.por_id.values().filter(|i| i.secao == "extras" && categorias.contains(&i.categoria.as_str())).collect()
    }

    /// Registros da classe File (0x0701–0x070F), mostrados na aba História.
    pub fn arquivos(&self) -> Vec<&Item> {
        self.por_id.values().filter(|i| i.categoria == "arquivo").collect()
    }
}

/// Rótulo em português de uma categoria.
pub fn nome_categoria(c: &str) -> &'static str {
    match c {
        "arma" => "Arma",
        "granada_ovo" => "Granada / ovo",
        "municao" => "Munição",
        "cura" => "Cura",
        "tesouro" => "Tesouro",
        "chave" => "Chave",
        "outros" => "Outros",
        "arquivo" => "Arquivo",
        "arma_inimigo" => "Arma de inimigo",
        "arma_dlc" => "Arma de DLC",
        "item_dlc" => "Item de DLC",
        "objeto_fase" => "Objeto de fase",
        "interno" => "Interno",
        "interno_dummy" => "Sem nome no jogo",
        _ => "Vazio",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const V1: &str = include_str!("../tests/dados/re5_items_v1.json");

    fn antigos() -> BTreeMap<u32, String> {
        #[derive(Deserialize)]
        struct Bruto {
            name: String,
        }
        let b: BTreeMap<String, Bruto> = serde_json::from_str(V1).unwrap();
        b.into_iter().map(|(k, v)| (u32::from_str_radix(k.trim_start_matches("0x"), 16).unwrap(), v.name)).collect()
    }

    #[test]
    fn tem_os_407_registros_por_classe() {
        let t = Itens::carrega();
        assert_eq!(t.todos().count(), 407);
        let por_classe = |c: usize| t.todos().filter(|i| classe_de(i.id) == c).count();
        assert_eq!([por_classe(1), por_classe(2), por_classe(3), por_classe(4), por_classe(5), por_classe(6), por_classe(7)],
                   [144, 18, 19, 129, 64, 16, 16]);
    }

    #[test]
    fn nomes_em_portugues_com_o_ingles() {
        let t = Itens::carrega();
        assert_eq!(t.nome(0x0305), "Erva verde (Herb (Green))");
        assert_eq!(t.nome(0x0451), "Rubi em gota (Ruby (Pear))");
        assert_eq!(t.nome(0x0102), "M92F (HG)"); // nome próprio: fica só o oficial
        assert_eq!(t.nome(0x0504), "Key_04"); // sem nome no jogo: o nome antigo
        // todo item que o editor já tinha continua com nome
        for id in antigos().into_keys().filter(|id| *id != 0) {
            assert!(t.item(id).unwrap().nome.is_some(), "0x{id:04x}");
        }
    }

    #[test]
    fn nada_novo_fica_gravavel() {
        // Graváveis + legado cabem na lista do editor antigo; só saíram os das DLCs.
        let t = Itens::carrega();
        let antigos: Vec<u32> = antigos().into_keys().filter(|id| *id != 0).collect();
        let agora: Vec<u32> = t.todos().filter(|i| i.gravavel || i.legado).map(|i| i.id).collect();
        let fora: Vec<u32> = antigos.iter().copied().filter(|id| !agora.contains(id)).collect();
        assert!(agora.iter().all(|id| antigos.contains(id)));
        assert!(fora.iter().all(|id| t.item(*id).unwrap().dlc.is_some()), "{fora:x?}");
        assert_eq!(fora.len(), 22);
        for i in t.todos().filter(|i| i.gravavel) {
            assert!(i.nome.is_some() && i.oficial.is_some(), "gravável sem nome: 0x{:04x}", i.id);
        }
    }

    #[test]
    fn inimigos_e_internos_nunca_sao_opcao() {
        let t = Itens::carrega();
        for c in 1..CLASSES.len() {
            for d in [Destino::Inventario, Destino::Personagem] {
                for id in t.opcoes(c, 0, d) {
                    let i = t.item(id).unwrap();
                    assert!(i.gravavel, "0x{id:04x}");
                    assert!(!matches!(i.categoria.as_str(), "arma_inimigo" | "objeto_fase" | "interno" | "interno_dummy" | "arma_dlc"));
                }
            }
        }
        assert!(t.opcoes(1, 0, Destino::Inventario).contains(&0x0102));
        assert!(!t.opcoes(1, 0, Destino::Inventario).contains(&0x0150));
        // a erva "de chão" não entra; a do inventário, sim
        assert!(!t.opcoes(3, 0, Destino::Inventario).contains(&0x0301));
        assert!(t.opcoes(3, 0, Destino::Inventario).contains(&0x0305));
    }

    #[test]
    fn slots_do_personagem_so_com_itens_de_slot() {
        let t = Itens::carrega();
        assert!(t.opcoes(1, 0, Destino::Personagem).contains(&0x0102));
        assert!(t.opcoes(4, 0, Destino::Personagem).is_empty());
        assert!(t.opcoes(5, 0, Destino::Personagem).is_empty());
        assert!(!t.opcoes(4, 0, Destino::Inventario).is_empty());
        assert_eq!(t.classes_para(Destino::Personagem), vec![0, 1, 2, 3, 6]);
    }

    #[test]
    fn id_atual_nunca_some_da_lista() {
        let t = Itens::carrega();
        // tesouro num slot do Chris (vindo de um save) continua aparecendo
        assert_eq!(t.opcoes(4, 0x0417, Destino::Personagem)[0], 0x0417);
        // registro sem nome (legado) continua aparecendo se já estiver no save
        assert_eq!(t.opcoes(5, 0x0504, Destino::Inventario)[0], 0x0504);
        assert!(!t.opcoes(5, 0, Destino::Inventario).contains(&0x0504));
    }

    #[test]
    fn icones_dentro_do_atlas() {
        let t = Itens::carrega();
        let originais: Vec<i32> =
            t.todos().filter(|i| i.icone_tipo.as_deref() == Some("original")).map(|i| i.icone).collect();
        let mut ord = originais.clone();
        ord.sort();
        ord.dedup();
        assert_eq!(ord.len(), originais.len(), "ícone original repetido");
        assert!(t.todos().all(|i| (i.icone >= 0) == i.icone_tipo.is_some()));
        assert!(t.todos().all(|i| i.icone < 174 + 12));
        for id in [0x0102, 0x0201, 0x0305, 0x0417, 0x0520, 0x0427] {
            assert_eq!(t.item(id).unwrap().icone_tipo.as_deref(), Some("original"), "0x{id:04x}");
        }
        // ovo de cura usa o ícone do mesmo ovo da classe arma
        assert_eq!(t.icone(0x030C), t.icone(0x013C));
        assert_eq!(t.item(0x0129).unwrap().icone_tipo.as_deref(), Some("silhueta"));
        assert_eq!(t.icone(0x0501), -1); // sem imagem no jogo
    }

    #[test]
    fn grupos_das_extras() {
        let t = Itens::carrega();
        assert_eq!(t.extras(0).len(), 38);
        assert_eq!(t.extras(1).len(), 9);
        assert_eq!(t.extras(2).len(), 24);
        assert_eq!(t.extras(3).len(), 25);
        assert!(t.extras(4).iter().all(|i| !i.gravavel));
        assert_eq!(t.arquivos().len(), 16);
        assert!(t.todos().filter(|i| i.secao == "extras").all(|i| !i.gravavel));
    }

    #[test]
    fn descricoes_so_nos_blocos_certos() {
        // o bloco de descrições das armas vai só até 0x014F
        let t = Itens::carrega();
        assert!(t.item(0x0151).unwrap().descricao.is_none());
        assert!(t.todos().filter(|i| i.categoria == "arma_inimigo").all(|i| i.descricao.is_none()));
        assert!(t.item(0x0106).unwrap().descricao.as_deref().unwrap().starts_with("Explosivo potente"));
        assert_eq!(t.item(0x0201).unwrap().descricao.as_deref(), Some("Munição para pistola."));
        assert!(t.item(0x0417).unwrap().descricao.is_some()); // tesouros também
    }

    #[test]
    fn itens_das_dlcs() {
        let t = Itens::carrega();
        assert_eq!(t.item(0x0520).unwrap().dlc.as_deref(), Some("Lost in Nightmares"));
        assert_eq!(t.item(0x0529).unwrap().dlc.as_deref(), Some("Desperate Escape"));
        assert_eq!(t.item(0x0520).unwrap().oficial.as_deref(), Some("Square crank"));
        // só existem com a DLC carregada: ficam em Itens Extras, sem gravar
        for id in (0x0520..=0x0532).chain(0x0427..=0x0429) {
            let i = t.item(id).unwrap();
            assert!(!i.gravavel && i.secao == "extras" && i.categoria == "item_dlc", "0x{id:04x}");
            assert!(!t.opcoes(classe_de(id), 0, Destino::Inventario).contains(&id));
        }
    }
}
