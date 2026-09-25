//! Conversão entre os dados do backend (save.rs) e o que a interface mostra.
//! Nada aqui abre, grava ou altera arquivo.

use crate::itens::Itens;
use crate::save::Slot;
use crate::{Lista, Opcao, SlotVisivel};
use serde_json::Value;
use slint::{ModelRc, SharedString, StandardListViewItem, VecModel};
use std::path::PathBuf;
use std::rc::Rc;

/// Uma lista de desbloqueios como o backend descreve.
#[derive(Clone)]
pub struct ListaDados {
    pub chave: String,
    pub titulo: String,
    pub opcoes: Vec<String>,
    pub marcados: Vec<bool>,
}

/// Tudo o que o backend informou sobre o save aberto.
#[derive(Clone)]
pub struct Retrato {
    pub caminho: PathBuf,
    pub xbox: bool,
    pub gold: u32,
    pub points: u32,
    pub data: [u32; 6],
    pub checksum_salvo: String,
    pub checksum_real: String,
    pub checksum_ok: bool,
    pub listas: Vec<ListaDados>,
    // Xbox
    pub profile_id: String,
    pub device_id: String,
    pub assinatura_ok: bool,
    pub assinado_por: String,
    pub inventario: Vec<Slot>,
    pub chris: Vec<Slot>,
    pub sheva: Vec<Slot>,
    // PC
    pub steam_id: String,
    // ambiente
    pub keyvault: Option<String>,
}

fn texto(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_string()
}

fn slots(v: &Value, k: &str) -> Vec<Slot> {
    serde_json::from_value(v[k].clone()).unwrap_or_default()
}

impl Retrato {
    /// Lê o JSON de `Save::info()`.
    pub fn de_info(v: &Value, keyvault: Option<String>) -> Self {
        let mut data = [0u32; 6];
        for (i, d) in data.iter_mut().enumerate() {
            *d = v["data"][i].as_u64().unwrap_or(0) as u32;
        }
        let listas = v["listas"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|l| ListaDados {
                        chave: texto(l, "chave"),
                        titulo: texto(l, "titulo"),
                        opcoes: serde_json::from_value(l["opcoes"].clone()).unwrap_or_default(),
                        marcados: serde_json::from_value(l["marcados"].clone()).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            caminho: PathBuf::from(texto(v, "caminho")),
            xbox: v["plataforma"] == "xbox",
            gold: v["gold"].as_u64().unwrap_or(0) as u32,
            points: v["points"].as_u64().unwrap_or(0) as u32,
            data,
            checksum_salvo: texto(v, "checksum_salvo"),
            checksum_real: texto(v, "checksum_real"),
            checksum_ok: v["checksum_ok"].as_bool().unwrap_or(false),
            listas,
            profile_id: texto(v, "profile_id"),
            device_id: texto(v, "device_id"),
            assinatura_ok: v["assinatura_ok"].as_bool().unwrap_or(false),
            assinado_por: texto(v, "assinado_por"),
            inventario: slots(v, "inventario"),
            chris: slots(v, "chris"),
            sheva: slots(v, "sheva"),
            steam_id: texto(v, "steam_id"),
            keyvault,
        }
    }

    pub fn plataforma(&self) -> &'static str {
        if self.xbox { "Xbox 360 · Title ID 434307D4" } else { "PC · Steam" }
    }

    pub fn assinatura(&self) -> String {
        match (self.assinatura_ok, self.assinado_por.is_empty()) {
            (true, _) => format!("válida · console {}", self.assinado_por),
            (false, true) => "inválida · sem console".into(),
            (false, false) => format!("inválida · {}", self.assinado_por),
        }
    }

    pub fn nota(&self) -> String {
        let backup = " Antes de gravar por cima, uma cópia vai para a pasta backups.";
        if !self.xbox {
            return format!("Save de PC: ao gravar, o checksum é recalculado e o arquivo é cifrado de novo.{backup}");
        }
        match &self.keyvault {
            Some(kv) => format!("Assinatura automática ativada com {kv}: o save sai pronto para o Xbox.{backup}"),
            None => format!(
                "Sem keyvault (console/kv.bin ao lado do programa): o save é gravado com checksum e hashes, mas sem assinatura — assine no Horizon.{backup}"
            ),
        }
    }
}

// ---------------------------------------------------------------- textos

/// 9794 -> "9.794"
pub fn milhar(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn data_texto(d: &[u32; 6]) -> String {
    format!("{:02}/{:02}/{:04} {:02}:{:02}:{:02}", d[2], d[1], d[0], d[3], d[4], d[5])
}

/// "dd/mm/aaaa hh:mm:ss" (segundos opcionais) -> [ano, mês, dia, h, min, s]
pub fn data_de_texto(t: &str) -> Option<[u32; 6]> {
    let t = t.trim();
    let (dia, hora) = t.split_once(' ').unwrap_or((t, "00:00:00"));
    let d: Vec<u32> = dia.split('/').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
    let h: Vec<u32> = hora.trim().split(':').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
    if d.len() != 3 || !(2..=3).contains(&h.len()) {
        return None;
    }
    let r = [d[2], d[1], d[0], h[0], h[1], *h.get(2).unwrap_or(&0)];
    let ok = (2000..=2099).contains(&r[0])
        && (1..=12).contains(&r[1])
        && (1..=31).contains(&r[2])
        && r[3] <= 23
        && r[4] <= 59
        && r[5] <= 59;
    ok.then_some(r)
}

// ---------------------------------------------------------------- modelos da interface

pub fn listas_ui(listas: &[ListaDados]) -> ModelRc<Lista> {
    let v: Vec<Lista> = listas
        .iter()
        .map(|l| {
            let marcados = l.marcados.iter().filter(|m| **m).count();
            let opcoes: Vec<Opcao> = l
                .opcoes
                .iter()
                .zip(&l.marcados)
                .map(|(n, m)| Opcao { nome: n.into(), marcado: *m })
                .collect();
            Lista {
                titulo: l.titulo.clone().into(),
                contagem: format!("{marcados}/{}", l.opcoes.len()).into(),
                opcoes: ModelRc::from(Rc::new(VecModel::from(opcoes))),
            }
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

pub fn slots_ui(itens: &Itens, atuais: &[Slot], originais: &[Slot]) -> ModelRc<SlotVisivel> {
    let v: Vec<SlotVisivel> = atuais
        .iter()
        .zip(originais)
        .map(|(s, o)| SlotVisivel {
            numero: s.slot as i32 + 1,
            nome: itens.nome(s.id).into(),
            classe: Itens::nome_classe(s.id).into(),
            qtd: if s.id == 0 { SharedString::new() } else { format!("× {}", milhar(s.amount as u64)).into() },
            vazio: s.id == 0,
            mudou: s != o,
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

/// Linhas da tabela do inventário para os espaços escolhidos pelo filtro.
pub fn tabela_ui(itens: &Itens, atuais: &[Slot], originais: &[Slot], visiveis: &[usize]) -> ModelRc<ModelRc<StandardListViewItem>> {
    let linhas: Vec<ModelRc<StandardListViewItem>> = visiveis
        .iter()
        .map(|&i| {
            let s = &atuais[i];
            let mudou = s != &originais[i];
            let celulas: Vec<StandardListViewItem> = vec![
                format!("{}{}", i + 1, if mudou { "  •" } else { "" }).as_str().into(),
                itens.nome(s.id).as_str().into(),
                Itens::nome_classe(s.id).into(),
                (if s.id == 0 { String::new() } else { milhar(s.amount as u64) }).as_str().into(),
            ];
            ModelRc::from(Rc::new(VecModel::from(celulas)))
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(linhas)))
}

/// Quais espaços do inventário cada filtro mostra.
/// 0 = todos, 1 = só os ocupados, 2.. = classe 1..
pub fn filtrar(atuais: &[Slot], filtro: usize) -> Vec<usize> {
    atuais
        .iter()
        .enumerate()
        .filter(|(_, s)| match filtro {
            0 => true,
            1 => s.id != 0,
            f => s.id != 0 && crate::itens::classe_de(s.id) == f - 1,
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn nomes_filtros() -> ModelRc<SharedString> {
    let mut v: Vec<SharedString> = vec!["Todos os espaços".into(), "Só os ocupados".into()];
    v.extend(crate::itens::CLASSES[1..].iter().map(|c| SharedString::from(*c)));
    ModelRc::from(Rc::new(VecModel::from(v)))
}

pub fn textos(v: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    let v: Vec<SharedString> = v.into_iter().map(SharedString::from).collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_ida_e_volta() {
        let d = [2026, 9, 24, 17, 5, 30];
        assert_eq!(data_de_texto(&data_texto(&d)), Some(d));
        assert_eq!(data_de_texto("24/09/2026 17:05"), Some([2026, 9, 24, 17, 5, 0]));
        assert_eq!(data_de_texto("31/13/2026 10:00:00"), None);
        assert_eq!(data_de_texto("abc"), None);
    }

    #[test]
    fn milhar_formata() {
        assert_eq!(milhar(0), "0");
        assert_eq!(milhar(999), "999");
        assert_eq!(milhar(9794), "9.794");
        assert_eq!(milhar(4294967295), "4.294.967.295");
    }
}
