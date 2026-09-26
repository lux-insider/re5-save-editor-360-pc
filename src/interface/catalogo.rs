//! Abas Itens Extras e História: só mostram dados do jogo, nunca gravam.

use super::{modelos, Ctx};
use crate::historia::{self, Texto};
use crate::itens::GRUPOS_EXTRAS;
use crate::{JanelaPrincipal, Ponte, TextoLista};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// O que cada linha da lista da História abre.
#[derive(Clone, Copy)]
enum Linha {
    Cabecalho,
    Texto(usize),
    Arquivos,
}

pub struct Catalogo {
    textos: Vec<Texto>,
    linhas: Vec<Linha>,
    extras_ids: Vec<u32>,
}

impl Catalogo {
    pub fn novo() -> Self {
        let textos = historia::carrega();
        let mut linhas = Vec::new();
        for (grupo, _) in GRUPOS_HISTORIA {
            linhas.push(Linha::Cabecalho);
            linhas.extend(textos.iter().enumerate().filter(|(_, t)| t.grupo == grupo).map(|(i, _)| Linha::Texto(i)));
        }
        linhas.push(Linha::Cabecalho);
        linhas.push(Linha::Arquivos);
        Self { textos, linhas, extras_ids: Vec::new() }
    }
}

const GRUPOS_HISTORIA: [(&str, &str); 3] = [
    ("biblioteca", "BIBLIOTECA"),
    ("documento", "DOCUMENTOS DAS FASES"),
    ("dlc", "DOCUMENTOS DAS DLCs"),
];

pub fn iniciar(ui: &JanelaPrincipal, ctx: &Ctx) {
    let p = ui.global::<Ponte>();
    let rotulos: Vec<String> = {
        let e = ctx.borrow();
        GRUPOS_EXTRAS.iter().enumerate().map(|(g, (n, _))| format!("{n} ({})", e.itens.extras(g).len())).collect()
    };
    p.set_extras_grupos(modelos::textos(rotulos));
    mostrar_extras(ui, ctx, 0);
    mostrar_historia(ui, ctx);
    escolher_texto(ui, ctx, 1);
}

// ---------------------------------------------------------------- itens extras

pub fn mostrar_extras(ui: &JanelaPrincipal, ctx: &Ctx, grupo: usize) {
    let p = ui.global::<Ponte>();
    let mut e = ctx.borrow_mut();
    let ids: Vec<u32> = e.itens.extras(grupo).iter().map(|i| i.id).collect();
    p.set_extras_itens(modelos::opcoes_ui(&e.itens, &ids));
    p.set_extras_grupo(grupo as i32);
    e.catalogo.extras_ids = ids;
    drop(e);
    escolher_extra(ui, ctx, 0);
}

pub fn escolher_extra(ui: &JanelaPrincipal, ctx: &Ctx, i: i32) {
    let p = ui.global::<Ponte>();
    let e = ctx.borrow();
    match e.catalogo.extras_ids.get(i.max(0) as usize) {
        Some(&id) if i >= 0 => {
            p.set_extras_sel(i);
            p.set_extras_ficha(modelos::ficha(&e.itens, id));
        }
        _ => p.set_extras_sel(-1),
    }
}

// ---------------------------------------------------------------- história

/// Situação de um arquivo da Biblioteca no save aberto: -1 sem save / não se
/// aplica, 0 bloqueado, 1 desbloqueado.
fn estado(ctx: &Ctx, t: &Texto) -> i32 {
    let e = ctx.borrow();
    let Some(a) = &e.atual else { return -1 };
    let (Some(n), true) = (t.numero, t.grupo == "biblioteca") else { return -1 };
    let Some(lista) = a.listas.iter().find(|l| l.chave == "arquivos") else { return -1 };
    historia::indice_desbloqueio(n).and_then(|i| lista.marcados.get(i)).map(|m| *m as i32).unwrap_or(-1)
}

/// Monta a lista da História (recalcula o desbloqueado/bloqueado).
pub fn mostrar_historia(ui: &JanelaPrincipal, ctx: &Ctx) {
    let linhas = ctx.borrow().catalogo.linhas.clone();
    let mut cab = GRUPOS_HISTORIA.iter().map(|(_, n)| *n).chain(["REGISTROS DE ARQUIVO (FILE)"]);
    let v: Vec<TextoLista> = linhas
        .iter()
        .map(|l| match *l {
            Linha::Cabecalho => TextoLista { titulo: cab.next().unwrap_or("").into(), cabecalho: true, estado: -1 },
            Linha::Texto(i) => {
                let t = ctx.borrow().catalogo.textos[i].clone();
                TextoLista { titulo: t.titulo_pt.clone().into(), cabecalho: false, estado: estado(ctx, &t) }
            }
            Linha::Arquivos => TextoLista { titulo: "File 01–0F".into(), cabecalho: false, estado: -1 },
        })
        .collect();
    ui.global::<Ponte>().set_historia_lista(ModelRc::from(Rc::new(VecModel::from(v))));
    let sel = ui.global::<Ponte>().get_historia_sel();
    if sel >= 0 {
        escolher_texto(ui, ctx, sel);
    }
}

pub fn escolher_texto(ui: &JanelaPrincipal, ctx: &Ctx, i: i32) {
    let p = ui.global::<Ponte>();
    let Some(linha) = ctx.borrow().catalogo.linhas.get(i.max(0) as usize).copied() else { return };
    let (titulo, subtitulo, paginas) = match linha {
        Linha::Cabecalho => return,
        Linha::Texto(t) => {
            let texto = ctx.borrow().catalogo.textos[t].clone();
            let sub = match texto.grupo.as_str() {
                "biblioteca" => {
                    let situacao = match estado(ctx, &texto) {
                        1 => " · desbloqueado neste save",
                        0 => " · bloqueado neste save",
                        _ => "",
                    };
                    format!("Biblioteca · arquivo {} de 12{situacao}", texto.numero.unwrap_or(0))
                }
                "documento" => "Documento encontrado nas fases".to_string(),
                _ => "Documento das DLCs (Lost in Nightmares)".to_string(),
            };
            let sub = if texto.titulo_pt == texto.titulo { sub } else { format!("{sub} · original: {}", texto.titulo) };
            (texto.titulo_pt.clone(), sub, texto.paginas_pt)
        }
        Linha::Arquivos => arquivos(ctx),
    };
    p.set_historia_sel(i);
    p.set_historia_titulo(titulo.into());
    p.set_historia_subtitulo(subtitulo.into());
    let v: Vec<SharedString> = paginas.into_iter().map(SharedString::from).collect();
    p.set_historia_paginas(ModelRc::from(Rc::new(VecModel::from(v))));
}

/// Página explicando os registros File 01–0F da tabela de itens.
fn arquivos(ctx: &Ctx) -> (String, String, Vec<String>) {
    let e = ctx.borrow();
    let lista: Vec<String> = e
        .itens
        .arquivos()
        .iter()
        .filter(|i| i.id != 0x0700)
        .map(|i| {
            let fases = match i.colocacoes {
                0 => "não aparece nas fases do jogo base".to_string(),
                1 => "1 vez nas fases".to_string(),
                n => format!("{n} vezes nas fases"),
            };
            format!("0x{:04X}  {}  —  {fases}", i.id, i.interno)
        })
        .collect();
    (
        "Registros File 01–0F".into(),
        "Classe 0x07 da tabela de itens · somente consulta".into(),
        vec![
            "São os papéis e cadernos que o jogador pega nas fases. O jogo não dá nome a eles, e o texto que abre \
             não depende do ID: quem escolhe o documento é o script da fase. Por isso eles não têm título aqui."
                .into(),
            "O File 01 é o \"manual\" genérico que abre as páginas do BSAA Training Manual nos tutoriais. \
             Os textos que os outros abrem estão nas listas acima."
                .into(),
            lista.join("\n"),
        ],
    )
}
