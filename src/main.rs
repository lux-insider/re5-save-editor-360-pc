// RE5 Save Editor 360+PC — janela (tao + wry). A lógica do save fica em save.rs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dialogo;
mod save;

use save::{Mudancas, Save};
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::http::{header::CONTENT_TYPE, Request, Response};
use wry::{DragDropEvent, WebViewBuilder};

const HTML: &str = include_str!("../ui/index.html");
const WALLPAPER: &[u8] = include_bytes!("../ui/re5-wallpaper.jpg");
const ICONE: &[u8] = include_bytes!("../ui/icone64.rgba");
const ITENS: &str = include_str!("re5_items.json");

enum Evento {
    Ipc(String),
    Soltou(PathBuf),
}

#[derive(Deserialize)]
struct Pedido {
    id: u64,
    cmd: String,
    #[serde(default)]
    args: Value,
}

// ---------------------------------------------------------------- pastas

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

fn pasta_do_exe() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| ".".into())
}

/// Keyvault do console: `console/kv.bin` ao lado do programa (portátil);
/// no Linux, também o do RE5 Save Editor em ~/horizon_final (só leitura).
fn keyvault() -> Option<PathBuf> {
    [pasta_do_exe().join("console").join("kv.bin"), home().join("horizon_final").join("console").join("kv.bin")]
        .into_iter()
        .find(|p| p.is_file())
}

/// Backups ao lado do programa; se a pasta não for gravável, na pasta de
/// dados do usuário.
fn pasta_backups() -> Result<PathBuf, String> {
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

/// Onde os diálogos começam: a pasta do último save ou um lugar provável.
fn pasta_inicial(atual: &Option<PathBuf>) -> PathBuf {
    if let Some(p) = atual.as_ref().and_then(|p| p.parent()) {
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

// ---------------------------------------------------------------- estado e comandos

struct App {
    atual: Option<PathBuf>,
    sujo: bool,
    dlg: dialogo::Dialogos,
}

impl App {
    fn abre(&mut self, p: PathBuf) -> Result<Value, String> {
        let s = Save::abre(&p)?;
        self.atual = Some(p);
        self.sujo = false;
        Ok(self.com_extras(s.info()))
    }

    fn com_extras(&self, mut v: Value) -> Value {
        v["keyvault"] = json!(keyvault().map(|p| p.display().to_string()));
        v
    }

    fn executa(&mut self, cmd: &str, args: &Value) -> Result<Value, String> {
        match cmd {
            "iniciar" => {
                let arg = std::env::args().skip(1).find(|a| !a.starts_with('-'));
                Ok(match arg.map(PathBuf::from).filter(|p| p.is_file()) {
                    Some(p) => self.abre(p).unwrap_or(Value::Null),
                    None => Value::Null,
                })
            }
            "itens" => serde_json::from_str::<Value>(ITENS).map_err(|e| e.to_string()),
            "abrir" => {
                match self.dlg.abrir("Abrir save do Resident Evil 5", &pasta_inicial(&self.atual)) {
                    Some(p) => self.abre(p),
                    None => Ok(Value::Null),
                }
            }
            "recarregar" => {
                let p = self.atual.clone().ok_or("nenhum save aberto")?;
                self.abre(p)
            }
            "ids" => {
                match self.dlg.abrir("Escolha um save do perfil / console de destino", &pasta_inicial(&self.atual)) {
                    Some(p) => {
                        let (perfil, device) = save::ids_de_outro(&p)?;
                        Ok(json!({ "profile_id": perfil, "device_id": device }))
                    }
                    None => Ok(Value::Null),
                }
            }
            "sujo" => {
                self.sujo = args.as_bool().unwrap_or(false);
                Ok(Value::Null)
            }
            "salvar" => self.salva(args),
            outro => Err(format!("comando desconhecido: {outro}")),
        }
    }

    fn salva(&mut self, args: &Value) -> Result<Value, String> {
        let atual = self.atual.clone().ok_or("nenhum save aberto")?;
        let m: Mudancas = serde_json::from_value(args["mudancas"].clone()).map_err(|e| e.to_string())?;
        let como = args["como"].as_bool().unwrap_or(false);

        let mut s = Save::abre(&atual)?;
        s.aplica(&m)?;
        let kv = keyvault();
        let (bytes, console) = s.monta(kv.as_deref())?;

        let destino = if como {
            let nome = atual.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("savedata.bin".into());
            match self.dlg.salvar("Salvar save como…", &pasta_inicial(&self.atual), &nome) {
                Some(p) => p,
                None => return Ok(Value::Null),
            }
        } else {
            atual
        };

        let mut backup = None;
        if destino.is_file() {
            let pasta = pasta_backups()?;
            let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let plat = if matches!(s.plat, save::Plataforma::Pc) { "pc" } else { "xbox" };
            let b = pasta.join(format!("{plat}_{}_{nome}", chrono::Local::now().format("%Y%m%d_%H%M%S")));
            std::fs::copy(&destino, &b).map_err(|e| format!("backup: {e}"))?;
            backup = Some(b.display().to_string());
        }
        std::fs::write(&destino, &bytes).map_err(|e| format!("{}: {e}", destino.display()))?;
        let ok = Save::confere(&destino, console.is_some());
        let info = self.abre(destino.clone())?;
        Ok(json!({ "ok": ok, "assinado_por": console, "backup": backup, "caminho": destino.display().to_string(), "info": info }))
    }
}

fn resposta(tipo: &'static str, corpo: Cow<'static, [u8]>) -> Response<Cow<'static, [u8]>> {
    Response::builder().header(CONTENT_TYPE, tipo).body(corpo).unwrap()
}

fn serve(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    match req.uri().path() {
        "/re5-wallpaper.jpg" => resposta("image/jpeg", Cow::Borrowed(WALLPAPER)),
        _ => resposta("text/html; charset=utf-8", Cow::Borrowed(HTML.as_bytes())),
    }
}

fn main() {
    let event_loop = EventLoopBuilder::<Evento>::with_user_event().build();
    let janela = WindowBuilder::new()
        .with_title("RE5 Save Editor 360+PC")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 860.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(760.0, 560.0))
        .with_window_icon(Icon::from_rgba(ICONE.to_vec(), 64, 64).ok())
        .build(&event_loop)
        .expect("não foi possível abrir a janela");

    let p_ipc = event_loop.create_proxy();
    let p_drop = event_loop.create_proxy();
    // No Windows o WebView2 só aceita esquemas próprios como http://<nome>.localhost
    let url = if cfg!(windows) { "http://re5.localhost/" } else { "re5://localhost/" };
    let builder = WebViewBuilder::new()
        .with_custom_protocol("re5".into(), move |_id, req| serve(req))
        .with_url(url)
        .with_ipc_handler(move |req: Request<String>| {
            let _ = p_ipc.send_event(Evento::Ipc(req.into_body()));
        })
        .with_drag_drop_handler(move |ev| {
            if let DragDropEvent::Drop { paths, .. } = ev {
                if let Some(p) = paths.into_iter().next() {
                    let _ = p_drop.send_event(Evento::Soltou(p));
                }
                return true;
            }
            false
        });

    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&janela).unwrap_or_else(|e| {
        dialogo::erro_fatal(&format!(
            "Não foi possível abrir a janela: falta o Microsoft Edge WebView2 Runtime.\n\n\
             Ele já vem no Windows 10 e 11. Se foi removido, instale de graça em:\n\
             https://developer.microsoft.com/microsoft-edge/webview2/\n\n({e})"
        ))
    });
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder
            .build_gtk(janela.default_vbox().unwrap())
            .unwrap_or_else(|e| dialogo::erro_fatal(&format!("Não foi possível abrir a janela (WebKitGTK): {e}")))
    };

    let mut app = App { atual: None, sujo: false, dlg: dialogo::Dialogos::novo(&janela) };
    event_loop.run(move |evento, _, controle| {
        *controle = ControlFlow::Wait;
        match evento {
            Event::UserEvent(Evento::Ipc(msg)) => {
                let Ok(p) = serde_json::from_str::<Pedido>(&msg) else { return };
                if std::env::var_os("RE5_DEBUG").is_some() {
                    eprintln!("pedido: {} {}", p.cmd, p.args);
                }
                let (ok, valor) = match app.executa(&p.cmd, &p.args) {
                    Ok(v) => (true, v),
                    Err(e) => (false, json!(e)),
                };
                let _ = webview.evaluate_script(&format!("window.__resposta({}, {ok}, {valor})", p.id));
            }
            Event::UserEvent(Evento::Soltou(p)) => {
                let js = match app.abre(p) {
                    Ok(info) => format!("window.__soltou(true, {info})"),
                    Err(e) => format!("window.__soltou(false, {})", json!(e)),
                };
                let _ = webview.evaluate_script(&js);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                let sair = !app.sujo
                    || app.dlg.confirmar("Sair sem salvar?", "Existem alterações que ainda não foram gravadas no save. Sair mesmo assim?");
                if sair {
                    *controle = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
