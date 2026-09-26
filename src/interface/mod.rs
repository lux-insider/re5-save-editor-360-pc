//! Controlador: o único lugar que liga a interface Slint ao backend.
//!
//! A interface (ui/*.slint) só mostra as propriedades do `Ponte` e chama os
//! callbacks dele. Aqui cada callback vira uma ação: ler ou gravar o save
//! (save.rs), atualizar o estado da edição e devolver o resultado à tela.

mod catalogo;
mod modelos;

use crate::itens::{classe_de, Destino, Itens, CLASSES};
use crate::save::{self, Mudancas, Save, Slot};
use crate::sistema::{dialogos, pastas};
use crate::{JanelaPrincipal, Ponte};
use modelos::{milhar, Retrato};
use slint::{ComponentHandle, SharedString};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Onde {
    Chris,
    Sheva,
    Inventario,
}

impl Onde {
    fn maximo(self) -> u32 {
        if self == Onde::Inventario { 0xFFFF } else { u32::MAX }
    }

    fn destino(self) -> Destino {
        if self == Onde::Inventario { Destino::Inventario } else { Destino::Personagem }
    }
}

/// Ação que espera a confirmação do usuário (há alterações não gravadas).
enum Pendente {
    Abrir,
    AbrirCaminho(PathBuf),
    Recarregar,
    Sair,
}

struct Estado {
    itens: Itens,
    /// Como o save está no arquivo.
    original: Option<Retrato>,
    /// A edição em andamento (listas e slots; os campos de texto ficam na tela).
    atual: Option<Retrato>,
    /// Espaços do inventário mostrados na tabela, na ordem das linhas.
    visiveis: Vec<usize>,
    editor: Option<(Onde, usize)>,
    editor_ids: Vec<u32>,
    /// Classes oferecidas no editor de slot, na ordem do ComboBox.
    editor_classes: Vec<usize>,
    /// Abas Itens Extras e História.
    catalogo: catalogo::Catalogo,
    pendente: Option<Pendente>,
    sujo: bool,
    avisos: u64,
}

type Ctx = Rc<RefCell<Estado>>;

/// Liga todos os callbacks da interface e, se houver, abre o save inicial.
pub fn iniciar(ui: &JanelaPrincipal, inicial: Option<PathBuf>) {
    let ctx: Ctx = Rc::new(RefCell::new(Estado {
        itens: Itens::carrega(),
        original: None,
        atual: None,
        visiveis: Vec::new(),
        editor: None,
        editor_ids: Vec::new(),
        editor_classes: Vec::new(),
        catalogo: catalogo::Catalogo::novo(),
        pendente: None,
        sujo: false,
        avisos: 0,
    }));
    let p = ui.global::<Ponte>();
    p.set_titulo(crate::sistema::chave_embutida::TITULO.into());
    p.set_classes(modelos::textos(CLASSES.iter().map(|c| c.to_string())));
    p.set_filtros(modelos::nomes_filtros());
    {
        let e = ctx.borrow();
        p.set_atlas_colunas(e.itens.colunas_atlas);
        p.set_atlas_celula(e.itens.celula_atlas);
    }
    catalogo::iniciar(ui, &ctx);

    // Cada callback recebe a janela (fraca) e o estado.
    macro_rules! liga {
        ($metodo:ident, |$ui:ident, $ctx:ident $(, $arg:ident : $tipo:ty)*| $corpo:expr) => {{
            let fraca = ui.as_weak();
            let ctx = ctx.clone();
            p.$metodo(move |$($arg: $tipo),*| {
                let Some($ui) = fraca.upgrade() else { return };
                let $ctx = &ctx;
                $corpo
            });
        }};
    }

    liga!(on_abrir, |ui, ctx| pedir(&ui, ctx, Pendente::Abrir));
    liga!(on_recarregar, |ui, ctx| pedir(&ui, ctx, Pendente::Recarregar));
    liga!(on_sair, |ui, ctx| pedir(&ui, ctx, Pendente::Sair));
    liga!(on_salvar, |ui, ctx| salvar(&ui, ctx, false));
    liga!(on_salvar_como, |ui, ctx| salvar(&ui, ctx, true));
    liga!(on_desfazer, |ui, ctx| {
        let original = ctx.borrow().original.clone();
        if let Some(r) = original {
            carregar(&ui, ctx, r);
            aviso(&ui, ctx, "Alterações desfeitas.", 0);
        }
    });
    liga!(on_campo_editado, |ui, ctx| {
        limpar_campos(&ui);
        atualizar(&ui, ctx);
    });
    liga!(on_copiar_ids, |ui, ctx| copiar_ids(&ui, ctx));
    liga!(on_voltar_ids, |ui, ctx| {
        if let Some(o) = &ctx.borrow().original {
            let p = ui.global::<Ponte>();
            p.set_profile_id(o.profile_id.clone().into());
            p.set_device_id(o.device_id.clone().into());
        }
        atualizar(&ui, ctx);
    });
    liga!(on_marcar, |ui, ctx, lista: i32, opcao: i32, valor: bool| {
        if let Some(a) = ctx.borrow_mut().atual.as_mut() {
            if let Some(m) = a.listas.get_mut(lista as usize).and_then(|l| l.marcados.get_mut(opcao as usize)) {
                *m = valor;
            }
        }
        mostrar_listas(&ui, ctx);
        catalogo::mostrar_historia(&ui, ctx);
        atualizar(&ui, ctx);
    });
    liga!(on_marcar_todos, |ui, ctx, lista: i32, valor: bool| {
        if let Some(l) = ctx.borrow_mut().atual.as_mut().and_then(|a| a.listas.get_mut(lista as usize)) {
            l.marcados.iter_mut().for_each(|m| *m = valor);
        }
        mostrar_listas(&ui, ctx);
        catalogo::mostrar_historia(&ui, ctx);
        atualizar(&ui, ctx);
    });
    liga!(on_selecionar_slot, |ui, ctx, onde: SharedString, indice: i32| {
        let onde = if onde == "sheva" { Onde::Sheva } else { Onde::Chris };
        abrir_editor(&ui, ctx, onde, indice as usize);
    });
    liga!(on_selecionar_linha, |ui, ctx, linha: i32| {
        let espaco = ctx.borrow().visiveis.get(linha.max(0) as usize).copied();
        if let (true, Some(i)) = (linha >= 0, espaco) {
            abrir_editor(&ui, ctx, Onde::Inventario, i);
        }
    });
    liga!(on_trocar_filtro, |ui, ctx, _filtro: i32| mostrar_slots(&ui, ctx));
    liga!(on_editor_classe_mudou, |ui, ctx, classe: i32| trocar_classe(&ui, ctx, classe.max(0) as usize));
    liga!(on_editor_escolher, |ui, ctx, i: i32| escolher_item(&ui, ctx, i.max(0) as usize));
    liga!(on_extras_trocar_grupo, |ui, ctx, g: i32| catalogo::mostrar_extras(&ui, ctx, g.max(0) as usize));
    liga!(on_extras_escolher, |ui, ctx, i: i32| catalogo::escolher_extra(&ui, ctx, i));
    liga!(on_historia_escolher, |ui, ctx, i: i32| catalogo::escolher_texto(&ui, ctx, i));
    liga!(on_aplicar_slot, |ui, ctx| aplicar_slot(&ui, ctx, false));
    liga!(on_esvaziar_slot, |ui, ctx| aplicar_slot(&ui, ctx, true));
    liga!(on_confirma_resposta, |ui, ctx, sim: bool| {
        ui.global::<Ponte>().set_confirma_visivel(false);
        let pendente = ctx.borrow_mut().pendente.take();
        if let (true, Some(acao)) = (sim, pendente) {
            executar(&ui, ctx, acao);
        }
    });
    liga!(on_fechar_aviso, |ui, _ctx| ui.global::<Ponte>().set_aviso_visivel(false));

    // Fechar a janela com alterações pendentes pede confirmação.
    {
        let fraca = ui.as_weak();
        let ctx = ctx.clone();
        ui.window().on_close_requested(move || {
            let Some(ui) = fraca.upgrade() else { return slint::CloseRequestResponse::HideWindow };
            if ctx.borrow().sujo {
                pedir(&ui, &ctx, Pendente::Sair);
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                slint::CloseRequestResponse::HideWindow
            }
        });
    }

    arrastar_e_soltar(ui, &ctx);

    if let Some(caminho) = inicial {
        abrir_caminho(ui, &ctx, caminho);
    }
}

// ---------------------------------------------------------------- abrir e mostrar

fn abrir_caminho(ui: &JanelaPrincipal, ctx: &Ctx, caminho: PathBuf) {
    match Save::abre(&caminho) {
        Ok(s) => {
            let kv = pastas::chave().map(|c| c.descricao());
            carregar(ui, ctx, Retrato::de_info(&s.info(), kv));
            aviso(ui, ctx, "Save aberto.", 0);
        }
        Err(e) => aviso(ui, ctx, &format!("Erro ao abrir: {e}"), 2),
    }
}

/// Mostra um save (recém-aberto ou desfeito) e zera a edição.
fn carregar(ui: &JanelaPrincipal, ctx: &Ctx, r: Retrato) {
    let p = ui.global::<Ponte>();
    p.set_plataforma(r.plataforma().into());
    p.set_caminho(r.caminho.display().to_string().into());
    p.set_gold(r.gold.to_string().into());
    p.set_points(r.points.to_string().into());
    p.set_profile_id(r.profile_id.clone().into());
    p.set_device_id(r.device_id.clone().into());
    p.set_steam_id(r.steam_id.clone().into());
    p.set_data(modelos::data_texto(&r.data).into());
    p.set_checksum_salvo(r.checksum_salvo.clone().into());
    p.set_checksum_real(r.checksum_real.clone().into());
    p.set_checksum_ok(r.checksum_ok);
    p.set_assinatura(r.assinatura().into());
    p.set_assinatura_ok(r.assinatura_ok);
    p.set_nota(r.nota().into());
    p.set_editor_ativo(false);
    p.set_filtro(0);
    p.set_aberto(true);
    p.set_xbox(r.xbox);
    // As abas mudam entre Xbox e PC: volta para a primeira.
    if r.xbox != p.get_xbox() || !p.get_aberto() {
        p.set_aba(0);
    }
    if p.get_lista_atual() as usize >= r.listas.len() {
        p.set_lista_atual(0);
    }
    {
        let mut e = ctx.borrow_mut();
        e.original = Some(r.clone());
        e.atual = Some(r);
        e.editor = None;
    }
    mostrar_listas(ui, ctx);
    mostrar_slots(ui, ctx);
    catalogo::mostrar_historia(ui, ctx);
    atualizar(ui, ctx);
}

fn mostrar_listas(ui: &JanelaPrincipal, ctx: &Ctx) {
    if let Some(a) = &ctx.borrow().atual {
        ui.global::<Ponte>().set_listas(modelos::listas_ui(&a.listas));
    }
}

fn mostrar_slots(ui: &JanelaPrincipal, ctx: &Ctx) {
    let p = ui.global::<Ponte>();
    let mut e = ctx.borrow_mut();
    let e = &mut *e;
    let (Some(a), Some(o)) = (&e.atual, &e.original) else { return };
    p.set_chris(modelos::slots_ui(&e.itens, &a.chris, &o.chris));
    p.set_sheva(modelos::slots_ui(&e.itens, &a.sheva, &o.sheva));
    e.visiveis = modelos::filtrar(&a.inventario, p.get_filtro().max(0) as usize);
    p.set_inventario(modelos::inventario_ui(&e.itens, &a.inventario, &o.inventario, &e.visiveis));
    let ocupados = a.inventario.iter().filter(|s| s.id != 0).count();
    p.set_resumo_inventario(
        format!("{} espaços · {ocupados} ocupados · mostrando {}", a.inventario.len(), e.visiveis.len()).into(),
    );
}

// ---------------------------------------------------------------- campos e validação

/// Tira do texto o que não pode estar nele (letras no dinheiro, etc.).
fn limpar_campos(ui: &JanelaPrincipal) {
    let p = ui.global::<Ponte>();
    let so_numeros = |s: SharedString| -> Option<SharedString> {
        let limpo: String = s.chars().filter(char::is_ascii_digit).collect();
        (limpo != s.as_str()).then(|| limpo.into())
    };
    let so_hex = |s: SharedString| -> Option<SharedString> {
        let limpo: String = s.chars().filter(char::is_ascii_hexdigit).map(|c| c.to_ascii_uppercase()).collect();
        (limpo != s.as_str()).then(|| limpo.into())
    };
    if let Some(v) = so_numeros(p.get_gold()) { p.set_gold(v); }
    if let Some(v) = so_numeros(p.get_points()) { p.set_points(v); }
    if let Some(v) = so_numeros(p.get_steam_id()) { p.set_steam_id(v); }
    if let Some(v) = so_hex(p.get_profile_id()) { p.set_profile_id(v); }
    if let Some(v) = so_hex(p.get_device_id()) { p.set_device_id(v); }
}

fn hex_ok(s: &str, tam: usize) -> bool {
    s.len() == tam && s.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Compara a tela com o arquivo: devolve as mudanças e se tudo é válido,
/// e marca na tela os campos inválidos.
fn ler_mudancas(ui: &JanelaPrincipal, ctx: &Ctx) -> Option<(Mudancas, bool)> {
    let p = ui.global::<Ponte>();
    let e = ctx.borrow();
    let (Some(o), Some(a)) = (&e.original, &e.atual) else { return None };
    let mut m = Mudancas::default();

    let gold = p.get_gold().parse::<u32>().ok();
    let points = p.get_points().parse::<u32>().ok();
    let data = modelos::data_de_texto(&p.get_data());
    p.set_gold_invalido(gold.is_none());
    p.set_points_invalido(points.is_none());
    p.set_data_invalida(data.is_none());
    let mut valido = gold.is_some() && points.is_some() && data.is_some();
    m.gold = gold.filter(|g| *g != o.gold);
    m.points = points.filter(|v| *v != o.points);
    m.data = data.filter(|d| *d != o.data);

    for (l, lo) in a.listas.iter().zip(&o.listas) {
        if l.marcados != lo.marcados {
            m.listas.push((l.chave.clone(), l.marcados.clone()));
        }
    }

    if o.xbox {
        let (perfil, device) = (p.get_profile_id().to_string(), p.get_device_id().to_string());
        p.set_profile_invalido(!hex_ok(&perfil, 16));
        p.set_device_invalido(!hex_ok(&device, 40));
        valido &= hex_ok(&perfil, 16) && hex_ok(&device, 40);
        m.profile_id = (perfil != o.profile_id).then_some(perfil);
        m.device_id = (device != o.device_id).then_some(device);
        let dif = |atual: &[Slot], orig: &[Slot]| -> Vec<Slot> {
            atual.iter().zip(orig).filter(|(s, so)| s != so).map(|(s, _)| s.clone()).collect()
        };
        m.inventario = dif(&a.inventario, &o.inventario);
        m.chris = dif(&a.chris, &o.chris);
        m.sheva = dif(&a.sheva, &o.sheva);
    } else {
        let steam = p.get_steam_id().to_string();
        let ok = !steam.is_empty() && steam.parse::<u64>().is_ok();
        p.set_steam_invalido(!ok);
        valido &= ok;
        m.steam_id = (steam != o.steam_id).then_some(steam);
    }
    Some((m, valido))
}

fn tem_mudanca(m: &Mudancas) -> bool {
    m.gold.is_some()
        || m.points.is_some()
        || m.data.is_some()
        || !m.listas.is_empty()
        || m.steam_id.is_some()
        || m.profile_id.is_some()
        || m.device_id.is_some()
        || !m.inventario.is_empty()
        || !m.chris.is_empty()
        || !m.sheva.is_empty()
}

/// Recalcula "tem alteração?" e "pode salvar?" depois de qualquer edição.
fn atualizar(ui: &JanelaPrincipal, ctx: &Ctx) {
    let (sujo, valido) = ler_mudancas(ui, ctx).map(|(m, v)| (tem_mudanca(&m), v)).unwrap_or((false, true));
    let p = ui.global::<Ponte>();
    p.set_sujo(sujo);
    p.set_valido(valido);
    p.set_pode_salvar(sujo && valido);
    ctx.borrow_mut().sujo = sujo;
}

// ---------------------------------------------------------------- editor de slot

fn slot_de(e: &Estado, onde: Onde, i: usize) -> Option<Slot> {
    let a = e.atual.as_ref()?;
    let v = match onde {
        Onde::Chris => &a.chris,
        Onde::Sheva => &a.sheva,
        Onde::Inventario => &a.inventario,
    };
    v.get(i).cloned()
}

fn abrir_editor(ui: &JanelaPrincipal, ctx: &Ctx, onde: Onde, i: usize) {
    let Some(s) = slot_de(&ctx.borrow(), onde, i) else { return };
    let p = ui.global::<Ponte>();
    let titulo = match onde {
        Onde::Chris => format!("Chris · slot {}", i + 1),
        Onde::Sheva => format!("Sheva · slot {}", i + 1),
        Onde::Inventario => format!("Inventário · espaço {}", i + 1),
    };
    p.set_editor_titulo(titulo.into());
    p.set_editor_onde(match onde { Onde::Chris => "chris", Onde::Sheva => "sheva", Onde::Inventario => "inventario" }.into());
    p.set_editor_indice(i as i32);
    p.set_editor_max(milhar(onde.maximo() as u64).into());
    p.set_editor_qtd(s.amount.to_string().into());
    p.set_editor_qtd_invalida(false);
    {
        // Classes que podem ir para esse destino, mais a do item atual.
        let mut e = ctx.borrow_mut();
        let mut classes = e.itens.classes_para(onde.destino());
        let atual = classe_de(s.id).min(CLASSES.len() - 1);
        if !classes.contains(&atual) {
            classes.push(atual);
            classes.sort();
        }
        p.set_classes(modelos::textos(classes.iter().map(|&c| CLASSES[c].to_string())));
        e.editor_classes = classes;
        e.editor = Some((onde, i));
    }
    preencher_itens(ui, ctx, classe_de(s.id).min(CLASSES.len() - 1), s.id);
    p.set_editor_ativo(true);
}

/// Preenche a grade de itens de uma classe e marca o item atual.
fn preencher_itens(ui: &JanelaPrincipal, ctx: &Ctx, classe: usize, atual: u32) {
    let p = ui.global::<Ponte>();
    let mut e = ctx.borrow_mut();
    let destino = e.editor.map(|(o, _)| o.destino()).unwrap_or(Destino::Inventario);
    let ids = e.itens.opcoes(classe, atual, destino);
    let escolhido = ids.iter().position(|&id| id == atual).unwrap_or(0);
    p.set_editor_itens(modelos::opcoes_ui(&e.itens, &ids));
    p.set_editor_classe(e.editor_classes.iter().position(|&c| c == classe).unwrap_or(0) as i32);
    p.set_editor_item(escolhido as i32);
    p.set_editor_ficha(modelos::ficha(&e.itens, ids.get(escolhido).copied().unwrap_or(0)));
    e.editor_ids = ids;
}

fn escolher_item(ui: &JanelaPrincipal, ctx: &Ctx, i: usize) {
    let e = ctx.borrow();
    let Some(&id) = e.editor_ids.get(i) else { return };
    let p = ui.global::<Ponte>();
    p.set_editor_item(i as i32);
    p.set_editor_ficha(modelos::ficha(&e.itens, id));
}

fn trocar_classe(ui: &JanelaPrincipal, ctx: &Ctx, indice: usize) {
    let Some(classe) = ctx.borrow().editor_classes.get(indice).copied() else { return };
    let atual = ctx.borrow().editor.and_then(|(onde, i)| slot_de(&ctx.borrow(), onde, i)).map(|s| s.id).unwrap_or(0);
    preencher_itens(ui, ctx, classe, atual);
    let p = ui.global::<Ponte>();
    if classe == 0 {
        p.set_editor_qtd("0".into());
    } else if p.get_editor_qtd().trim() == "0" || p.get_editor_qtd().is_empty() {
        p.set_editor_qtd("1".into());
    }
}

fn aplicar_slot(ui: &JanelaPrincipal, ctx: &Ctx, esvaziar: bool) {
    let p = ui.global::<Ponte>();
    let Some((onde, i)) = ctx.borrow().editor else { return };
    let (id, qtd) = if esvaziar {
        (0, 0)
    } else {
        let id = ctx.borrow().editor_ids.get(p.get_editor_item().max(0) as usize).copied().unwrap_or(0);
        let qtd = p.get_editor_qtd().trim().parse::<u64>().ok().filter(|q| *q <= onde.maximo() as u64);
        let Some(qtd) = qtd else {
            p.set_editor_qtd_invalida(true);
            return;
        };
        (id, if id == 0 { 0 } else { qtd as u32 })
    };
    p.set_editor_qtd_invalida(false);
    {
        let mut e = ctx.borrow_mut();
        let Some(a) = e.atual.as_mut() else { return };
        let v = match onde {
            Onde::Chris => &mut a.chris,
            Onde::Sheva => &mut a.sheva,
            Onde::Inventario => &mut a.inventario,
        };
        if let Some(s) = v.get_mut(i) {
            s.id = id;
            s.amount = qtd;
        }
    }
    mostrar_slots(ui, ctx);
    abrir_editor(ui, ctx, onde, i);
    atualizar(ui, ctx);
}

// ---------------------------------------------------------------- IDs de outro save

fn copiar_ids(ui: &JanelaPrincipal, ctx: &Ctx) {
    let inicio = pastas::pasta_inicial(ctx.borrow().original.as_ref().map(|o| o.caminho.as_path()));
    let Some(arquivo) = dialogos::abrir("Escolha um save do perfil / console de destino", &inicio) else { return };
    match save::ids_de_outro(&arquivo) {
        Ok((perfil, device)) => {
            let p = ui.global::<Ponte>();
            p.set_profile_id(perfil.into());
            p.set_device_id(device.into());
            atualizar(ui, ctx);
            aviso(ui, ctx, "IDs copiados. Clique em Salvar arquivo para gravar.", 0);
        }
        Err(e) => aviso(ui, ctx, &format!("Erro: {e}"), 2),
    }
}

// ---------------------------------------------------------------- salvar

fn salvar(ui: &JanelaPrincipal, ctx: &Ctx, como: bool) {
    let Some((m, valido)) = ler_mudancas(ui, ctx) else { return };
    if !valido || (!como && !tem_mudanca(&m)) {
        return;
    }
    let atual = match &ctx.borrow().original {
        Some(o) => o.caminho.clone(),
        None => return,
    };
    match gravar(&atual, &m, como) {
        Ok(None) => {}
        Ok(Some(r)) => {
            abrir_caminho(ui, ctx, r.destino.clone());
            let mut texto = match (&r.ok, &r.assinado_por) {
                (true, Some(c)) => format!("✔ Save gravado, assinado (console {c}) e conferido."),
                (true, None) => "✔ Save gravado e conferido.".to_string(),
                (false, _) => "⚠ O save foi gravado, mas a conferência falhou. O backup está guardado.".to_string(),
            };
            texto.push_str(&format!("\n{}", r.destino.display()));
            if let Some(b) = &r.backup {
                texto.push_str(&format!("\nBackup: {}", b.display()));
            }
            aviso(ui, ctx, &texto, if r.ok { 1 } else { 2 });
        }
        Err(e) => aviso(ui, ctx, &format!("Erro ao salvar: {e}"), 2),
    }
}

struct Gravado {
    destino: PathBuf,
    backup: Option<PathBuf>,
    assinado_por: Option<String>,
    ok: bool,
}

/// Mesmo processo de antes: aplica, monta (e assina), faz o backup, grava e confere.
fn gravar(atual: &std::path::Path, m: &Mudancas, como: bool) -> Result<Option<Gravado>, String> {
    let mut s = Save::abre(atual)?;
    s.aplica(m)?;
    // O save de PC não é assinado: a chave só é lida no Xbox.
    let chave = match s.plat {
        save::Plataforma::Xbox(_) => pastas::chave().map(|c| c.carregar()).transpose()?,
        save::Plataforma::Pc => None,
    };
    let (bytes, assinado_por) = s.monta_com(chave.as_ref())?;

    let destino = if como {
        let nome = atual.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("savedata.bin".into());
        match dialogos::salvar("Salvar save como…", &pastas::pasta_inicial(Some(atual)), &nome) {
            Some(p) => p,
            None => return Ok(None),
        }
    } else {
        atual.to_path_buf()
    };

    let mut backup = None;
    if destino.is_file() {
        let pasta = pastas::pasta_backups()?;
        let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let plat = if matches!(s.plat, save::Plataforma::Pc) { "pc" } else { "xbox" };
        let b = pasta.join(format!("{plat}_{}_{nome}", chrono::Local::now().format("%Y%m%d_%H%M%S")));
        std::fs::copy(&destino, &b).map_err(|e| format!("backup: {e}"))?;
        backup = Some(b);
    }
    std::fs::write(&destino, &bytes).map_err(|e| format!("{}: {e}", destino.display()))?;
    let ok = Save::confere(&destino, assinado_por.is_some());
    Ok(Some(Gravado { destino, backup, assinado_por, ok }))
}

// ---------------------------------------------------------------- confirmações

/// Pede confirmação se houver alterações; senão executa na hora.
fn pedir(ui: &JanelaPrincipal, ctx: &Ctx, acao: Pendente) {
    if !ctx.borrow().sujo {
        executar(ui, ctx, acao);
        return;
    }
    let (titulo, texto, sim) = match &acao {
        Pendente::Sair => ("Sair sem salvar?", "Existem alterações que ainda não foram gravadas no save.", "SAIR SEM SALVAR"),
        Pendente::Recarregar => ("Recarregar o save?", "As alterações que ainda não foram gravadas serão perdidas.", "RECARREGAR"),
        _ => ("Abrir outro save?", "Existem alterações que ainda não foram gravadas no save.", "ABRIR MESMO ASSIM"),
    };
    let p = ui.global::<Ponte>();
    p.set_confirma_titulo(titulo.into());
    p.set_confirma_texto(texto.into());
    p.set_confirma_sim(sim.into());
    p.set_confirma_visivel(true);
    ctx.borrow_mut().pendente = Some(acao);
}

fn executar(ui: &JanelaPrincipal, ctx: &Ctx, acao: Pendente) {
    let caminho_atual = ctx.borrow().original.as_ref().map(|o| o.caminho.clone());
    match acao {
        Pendente::Sair => {
            ctx.borrow_mut().sujo = false;
            let _ = ui.hide();
            let _ = slint::quit_event_loop();
        }
        Pendente::Recarregar => {
            if let Some(c) = caminho_atual {
                abrir_caminho(ui, ctx, c);
            }
        }
        Pendente::AbrirCaminho(c) => abrir_caminho(ui, ctx, c),
        Pendente::Abrir => {
            let inicio = pastas::pasta_inicial(caminho_atual.as_deref());
            if let Some(c) = dialogos::abrir("Abrir save do Resident Evil 5", &inicio) {
                abrir_caminho(ui, ctx, c);
            }
        }
    }
}

// ---------------------------------------------------------------- avisos

/// Mostra um aviso no rodapé; tipo 0 = informação, 1 = sucesso, 2 = erro.
fn aviso(ui: &JanelaPrincipal, ctx: &Ctx, texto: &str, tipo: i32) {
    let p = ui.global::<Ponte>();
    p.set_aviso(texto.into());
    p.set_aviso_tipo(tipo);
    p.set_aviso_visivel(true);
    let n = {
        let mut e = ctx.borrow_mut();
        e.avisos += 1;
        e.avisos
    };
    let segundos = if tipo == 0 { 4 } else { 9 };
    let fraca = ui.as_weak();
    let ctx = ctx.clone();
    slint::Timer::single_shot(Duration::from_secs(segundos), move || {
        if let (Some(ui), true) = (fraca.upgrade(), ctx.borrow().avisos == n) {
            ui.global::<Ponte>().set_aviso_visivel(false);
        }
    });
}

// ---------------------------------------------------------------- arrastar o save para a janela

fn arrastar_e_soltar(ui: &JanelaPrincipal, ctx: &Ctx) {
    use slint::winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor};
    let fraca = ui.as_weak();
    let ctx = ctx.clone();
    ui.window().on_winit_window_event(move |_janela, evento| {
        let Some(ui) = fraca.upgrade() else { return EventResult::Propagate };
        let p = ui.global::<Ponte>();
        match evento {
            WindowEvent::HoveredFile(_) => p.set_arrastando(true),
            WindowEvent::HoveredFileCancelled => p.set_arrastando(false),
            WindowEvent::DroppedFile(caminho) => {
                p.set_arrastando(false);
                pedir(&ui, &ctx, Pendente::AbrirCaminho(caminho.clone()));
            }
            _ => {}
        }
        EventResult::Propagate
    });
}
