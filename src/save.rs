//! Save do Resident Evil 5: Xbox 360 e PC (Steam).
//!
//! Os dois guardam o mesmo `savedata.bin` interno, com as mesmas regiões de
//! checksum ([0x10, 0x3BAC) e [0x3BB0, 0x52B0)). As diferenças:
//!
//! * Xbox 360: o arquivo é um pacote STFS "CON" com o savedata.bin dentro,
//!   sem criptografia, big-endian. Ao gravar: checksum, hashes SHA-1 do
//!   pacote e, com o keyvault do console, a assinatura RSA.
//! * PC: o próprio arquivo é o savedata.bin, little-endian, cifrado com XOR
//!   encadeado em blocos de 8 bytes (formato descoberto por shinneider,
//!   github.com/shinneider/RE5-Save-Editor).
//!
//! Os campos do PC ficam 0xB4 bytes adiante dos do Xbox (dinheiro 0x194 no
//! PC e 0xE0 no Xbox); as roupas, filtros, munição infinita, arquivos e
//! figuras seguem o mesmo deslocamento — conferido no save do Xbox, em que
//! 0x7C tem exatamente 19 bits (as 19 armas) e 0x80 tem 12 (os 12 arquivos).

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

const TAM_MINIMO: usize = 0x52B0;
const OFF_CHECKSUM: usize = 0x08;
const OFF_DATA: usize = 0x10;

// ---------------------------------------------------------------- listas

/// Listas de desbloqueio (bit N = item N a partir do fim da lista, como no
/// editor de PC do shinneider).
pub struct Lista {
    pub chave: &'static str,
    pub titulo: &'static str,
    /// deslocamento a partir do início do bloco (Xbox 0x70 / PC 0x124)
    pub rel: usize,
    /// 64 bits (duas palavras: baixa, depois alta)
    pub dupla: bool,
    pub nomes: &'static [&'static str],
}

pub const LISTAS: &[Lista] = &[
    Lista { chave: "chris_roupas", titulo: "Roupas do Chris", rel: 0x00, dupla: false,
        nomes: &["Warrior", "S.T.A.R.S.", "Safari", "BSAA"] },
    Lista { chave: "sheva_roupas", titulo: "Roupas da Sheva", rel: 0x04, dupla: false,
        nomes: &["Business", "Tribal", "Clubbin'", "BSAA"] },
    Lista { chave: "filtros", titulo: "Filtros de tela", rel: 0x08, dupla: false,
        nomes: &["Ruído", "Retrô", "Clássico", "Padrão"] },
    Lista { chave: "municao", titulo: "Munição infinita", rel: 0x0C, dupla: false,
        nomes: &[
            "RPG", "S&W M500", "L. Hawk", "S&W M29", "H&K PSG-1", "Dragunov SVD", "S75",
            "SIG 556", "H&K MP5", "AK-74", "VZ61", "Hydra", "Jail Breaker", "M3", "Ithaca M37",
            "M93R", "SIG P226", "H&K P8", "M92F",
        ] },
    Lista { chave: "arquivos", titulo: "Arquivos (Biblioteca)", rel: 0x10, dupla: false,
        nomes: &[
            "Albert Wesker", "Excella Gionne", "Jill Valentine", "Tricell", "U-8",
            "Tribo Ndipaya", "Ricardo Irving", "Sheva Alomar", "Chris Redfield", "Majini",
            "BSAA", "A Saga de Resident Evil",
        ] },
    Lista { chave: "figuras", titulo: "Figuras", rel: 0x14, dupla: true,
        nomes: &[
            "Jill (rara)", "Sheva (rara)", "Chris (rara)", "Irving (transformado)", "Ndesu",
            "Popokarimu", "U-8", "Uroboros Aheri", "Crocodilo", "Adjule", "Bui Kichwa",
            "Kipepeo", "Licker β", "Uroboros", "Majini de moto", "Majini da metralhadora",
            "Majini da motosserra", "Executioner Majini", "Big Man Majini", "Reaper",
            "Majini (Duvalia)", "Majini (Base B)", "Majini (Base A)", "Majini gigante",
            "Majini (Pântano C)", "Majini (Pântano B)", "Majini (Pântano A)", "Majini (Agitador)",
            "Majini (Cephalo)", "Majini (Vila D)", "Majini (Vila C)", "Majini (Vila B)",
            "Majini (Vila A)", "Reynard", "Kirk", "Dave", "DeChant", "Spencer", "Irving",
            "Excella", "Wesker", "Jill (controlada)", "Josh", "Sheva (BSAA)", "Chris (BSAA)",
        ] },
];

// ---------------------------------------------------------------- utilitários

fn sha1(d: &[u8]) -> [u8; 20] {
    Sha1::digest(d).into()
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn de_hex(s: &str, tam: usize, nome: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() != tam * 2 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{nome} precisa ter {} dígitos hexadecimais", tam * 2));
    }
    Ok((0..tam).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect())
}

/// Números do XeCrypt ficam em qwords na ordem inversa.
fn qwrev(b: &[u8]) -> Vec<u8> {
    b.chunks(8).rev().flatten().copied().collect()
}

fn fixo_be(n: &BigUint, len: usize) -> Vec<u8> {
    let b = n.to_bytes_be();
    let mut out = vec![0u8; len.saturating_sub(b.len())];
    out.extend_from_slice(&b);
    out
}

// ---------------------------------------------------------------- savedata.bin (comum)

/// O savedata.bin decifrado, com a ordem dos bytes e os endereços de cada
/// plataforma.
pub struct Interno {
    pub d: Vec<u8>,
    pub big_endian: bool,
    off_gold: usize,
    off_listas: usize,
}

impl Interno {
    fn xbox(d: Vec<u8>) -> Self {
        Self { d, big_endian: true, off_gold: 0xE0, off_listas: 0x70 }
    }
    fn pc(d: Vec<u8>) -> Self {
        Self { d, big_endian: false, off_gold: 0x194, off_listas: 0x124 }
    }

    pub fn u32(&self, o: usize) -> u32 {
        let b: [u8; 4] = self.d[o..o + 4].try_into().unwrap();
        if self.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
    }

    fn set32(&mut self, o: usize, v: u32) {
        let b = if self.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.d[o..o + 4].copy_from_slice(&b);
    }

    pub fn checksum(&self) -> u32 {
        let a = (0..3815).map(|i| self.u32(0x10 + i * 4));
        let b = (0..1472).map(|i| self.u32(0x3BB0 + i * 4));
        a.chain(b).fold(0u32, u32::wrapping_add)
    }

    pub fn checksum_salvo(&self) -> u32 {
        self.u32(OFF_CHECKSUM)
    }

    fn fecha_checksum(&mut self) {
        let c = self.checksum();
        self.set32(OFF_CHECKSUM, c);
    }

    fn bits(&self, l: &Lista) -> u64 {
        let o = self.off_listas + l.rel;
        let baixo = self.u32(o) as u64;
        if l.dupla { baixo | (self.u32(o + 4) as u64) << 32 } else { baixo }
    }

    fn set_bits(&mut self, l: &Lista, v: u64) {
        let o = self.off_listas + l.rel;
        self.set32(o, v as u32);
        if l.dupla {
            self.set32(o + 4, (v >> 32) as u32);
        }
    }

    /// bit do item `i` da lista (o último da lista é o bit 0)
    fn bit_de(l: &Lista, i: usize) -> u64 {
        1u64 << (l.nomes.len() - 1 - i)
    }

    fn listas_json(&self) -> Value {
        LISTAS
            .iter()
            .map(|l| {
                let v = self.bits(l);
                let marcados: Vec<bool> = (0..l.nomes.len()).map(|i| v & Self::bit_de(l, i) != 0).collect();
                json!({ "chave": l.chave, "titulo": l.titulo, "opcoes": l.nomes, "marcados": marcados })
            })
            .collect()
    }

    fn comum_json(&self) -> Value {
        let data: Vec<u32> = (0..6).map(|i| self.u32(OFF_DATA + i * 4)).collect();
        json!({
            "data": data,
            "gold": self.u32(self.off_gold),
            "points": self.u32(self.off_gold + 4),
            "checksum_salvo": format!("{:08X}", self.checksum_salvo()),
            "checksum_real": format!("{:08X}", self.checksum()),
            "checksum_ok": self.checksum_salvo() == self.checksum(),
            "listas": self.listas_json(),
        })
    }

    fn aplica_comum(&mut self, m: &Mudancas) -> Result<()> {
        if let Some(g) = m.gold {
            self.set32(self.off_gold, g);
        }
        if let Some(p) = m.points {
            self.set32(self.off_gold + 4, p);
        }
        if let Some(dt) = &m.data {
            let [ano, mes, dia, h, min, s] = *dt;
            if !(2000..=2099).contains(&ano) || !(1..=12).contains(&mes) || !(1..=31).contains(&dia) || h > 23 || min > 59 || s > 59 {
                return Err("data do save inválida".into());
            }
            for (i, v) in dt.iter().enumerate() {
                self.set32(OFF_DATA + i * 4, *v);
            }
        }
        for (chave, marcados) in &m.listas {
            let l = LISTAS.iter().find(|l| l.chave == chave).ok_or(format!("lista desconhecida: {chave}"))?;
            if marcados.len() != l.nomes.len() {
                return Err(format!("{}: quantidade de opções errada", l.titulo));
            }
            // só mexe nos bits conhecidos; os outros ficam como estavam
            let mut v = self.bits(l);
            for (i, &on) in marcados.iter().enumerate() {
                let b = Self::bit_de(l, i);
                if on { v |= b } else { v &= !b }
            }
            self.set_bits(l, v);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- mudanças vindas da tela

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Slot {
    pub slot: usize,
    pub id: u32,
    pub amount: u32,
}

#[derive(Deserialize, Default, Debug)]
pub struct Mudancas {
    pub gold: Option<u32>,
    pub points: Option<u32>,
    pub data: Option<[u32; 6]>,
    #[serde(default)]
    pub listas: Vec<(String, Vec<bool>)>,
    pub steam_id: Option<String>,
    pub profile_id: Option<String>,
    pub device_id: Option<String>,
    #[serde(default)]
    pub inventario: Vec<Slot>,
    #[serde(default)]
    pub chris: Vec<Slot>,
    #[serde(default)]
    pub sheva: Vec<Slot>,
}

// ---------------------------------------------------------------- PC

const XOR_INICIAL: [u8; 8] = [0x00, 0x21, 0x11, 0x08, 0xC0, 0x4B, 0x00, 0x00];

fn pc_decifra(cifrado: &[u8]) -> Vec<u8> {
    let mut chave = XOR_INICIAL.to_vec();
    let mut out = Vec::with_capacity(cifrado.len());
    for bloco in cifrado.chunks(8) {
        out.extend(bloco.iter().zip(&chave).map(|(a, b)| a ^ b));
        chave = bloco.to_vec();
    }
    out
}

fn pc_cifra(claro: &[u8]) -> Vec<u8> {
    let mut chave = XOR_INICIAL.to_vec();
    let mut out = Vec::with_capacity(claro.len());
    for bloco in claro.chunks(8) {
        let c: Vec<u8> = bloco.iter().zip(&chave).map(|(a, b)| a ^ b).collect();
        out.extend_from_slice(&c);
        chave = c;
    }
    out
}

// ---------------------------------------------------------------- Xbox 360: STFS e assinatura

const PKG_PERFIL: (usize, usize) = (0x371, 8);
const PKG_DEVICE: (usize, usize) = (0x3FD, 0x14);
const VD: usize = 0x379;
const NOME_INTERNO: &[u8] = b"savedata.bin";

const XBOX_INV: usize = 0x3158;
const XBOX_INV_SLOTS: usize = 84;
const XBOX_CHRIS: usize = 0x3690;
const XBOX_SHEVA: usize = 0x3AB0;
const XBOX_SLOT_PERS: usize = 0x2C;
const XBOX_PERS_VISIVEIS: usize = 9;

const DIGEST_INFO: [u8; 15] = [0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04, 0x14];

pub fn e_pacote_stfs(d: &[u8]) -> bool {
    d.len() >= 0x1000 && matches!(&d[..4], b"CON " | b"LIVE" | b"PIRS")
}

/// Chave RSA e certificado do console, lidos do keyvault decifrado (kv.bin).
pub struct ChaveConsole {
    cert: Vec<u8>,
    n: BigUint,
    d: BigUint,
    pub console: String,
}

impl ChaveConsole {
    pub fn carrega(p: &Path) -> Result<Self> {
        let kv = fs::read(p).map_err(|e| format!("keyvault: {e}"))?;
        if kv.len() < 0x4000 {
            return Err("keyvault inválido".into());
        }
        let cert = kv[0x9C8..0x9C8 + 0x1A8].to_vec();
        let k = &kv[0x298..0x298 + 0x1D0];
        let e = BigUint::from(be32(k, 4));
        let n = BigUint::from_bytes_be(&qwrev(&k[0x10..0x90]));
        let p_ = BigUint::from_bytes_be(&qwrev(&k[0x90..0xD0]));
        let q = BigUint::from_bytes_be(&qwrev(&k[0xD0..0x110]));
        if &p_ * &q != n || u16::from_be_bytes([cert[0], cert[1]]) != 0x1A8 {
            return Err("keyvault inválido".into());
        }
        let um = BigUint::from(1u32);
        let d = e.modinv(&((&p_ - &um) * (&q - &um))).ok_or("keyvault inválido")?;
        let console = String::from_utf8_lossy(&cert[7..0x12]).into_owned();
        Ok(Self { cert, n, d, console })
    }

    fn assina(&self, dados: &[u8]) -> Vec<u8> {
        let mut t = DIGEST_INFO.to_vec();
        t.extend_from_slice(&sha1(dados));
        let mut m = vec![0x00, 0x01];
        m.extend(std::iter::repeat(0xFF).take(0x80 - 3 - t.len()));
        m.push(0x00);
        m.extend(t);
        let mut s = fixo_be(&BigUint::from_bytes_be(&m).modpow(&self.d, &self.n), 0x80);
        s.reverse();
        s
    }
}

/// Confere a assinatura do pacote com a chave pública do próprio certificado.
pub fn assinatura_ok(pkg: &[u8]) -> bool {
    let cert = &pkg[4..4 + 0x1A8];
    let e = BigUint::from(be32(cert, 0x24));
    let n = BigUint::from_bytes_be(&qwrev(&cert[0x28..0xA8]));
    if n == BigUint::from(0u32) {
        return false;
    }
    let mut sig = pkg[0x1AC..0x22C].to_vec();
    sig.reverse();
    let m = fixo_be(&BigUint::from_bytes_be(&sig).modpow(&e, &n), 0x80);
    let mut fim = DIGEST_INFO.to_vec();
    fim.extend_from_slice(&sha1(&pkg[0x22C..0x344]));
    m.ends_with(&fim)
}

pub fn ids_do_pacote(pkg: &[u8]) -> (String, String) {
    let (po, pl) = PKG_PERFIL;
    let (dof, dl) = PKG_DEVICE;
    (hex(&pkg[po..po + pl]), hex(&pkg[dof..dof + dl]))
}

/// Pacote STFS com um nível de hash (saves pequenos).
pub struct Stfs {
    pub d: Vec<u8>,
    base: usize,
    shift: u32,
    tabela: usize,
}

impl Stfs {
    pub fn novo(d: Vec<u8>) -> Result<Self> {
        if !e_pacote_stfs(&d) {
            return Err("não é um pacote STFS (CON/LIVE/PIRS)".into());
        }
        let tam_cab = be32(&d, 0x340) as usize;
        let sep = d[VD + 2];
        let base = (tam_cab + 0xFFF) & 0xF000;
        let shift = if base == 0xB000 || sep & 1 == 0 { 1 } else { 0 };
        // a tabela de hash ativa é a segunda cópia quando o bit 1 está ligado
        let tabela = base + if sep & 2 != 0 { 0x1000 } else { 0 };
        if be32(&d, VD + 0x1C) >= 0xAA {
            return Err("pacote grande demais (mais de um nível de hash)".into());
        }
        if d.len() < tabela + 0x1000 {
            return Err("pacote truncado".into());
        }
        Ok(Self { d, base, shift, tabela })
    }

    fn off(&self, b: usize) -> usize {
        self.base + ((b + (((b + 0xAA) / 0xAA) << self.shift)) << 12)
    }

    fn entrada(&self, b: usize) -> usize {
        self.tabela + b * 0x18
    }

    fn proximo(&self, b: usize) -> usize {
        let e = self.entrada(b);
        let x = &self.d[e + 0x15..e + 0x18];
        (x[0] as usize) << 16 | (x[1] as usize) << 8 | x[2] as usize
    }

    fn bloco(&self, b: usize) -> Result<&[u8]> {
        let o = self.off(b);
        self.d.get(o..o + 0x1000).ok_or_else(|| "bloco fora do pacote".to_string())
    }

    fn blocos_do_arquivo(&self) -> Result<(Vec<usize>, usize)> {
        let ft = u32::from_le_bytes([self.d[VD + 5], self.d[VD + 6], self.d[VD + 7], 0]) as usize;
        for ent in self.bloco(ft)?.chunks(0x40) {
            let len = (ent[0x28] & 0x3F) as usize;
            if len > 0 && &ent[..len] == NOME_INTERNO {
                let mut b = u32::from_le_bytes([ent[0x2F], ent[0x30], ent[0x31], 0]) as usize;
                let tam = be32(ent, 0x34) as usize;
                let mut blocos = Vec::new();
                for _ in 0..(tam + 0xFFF) / 0x1000 {
                    self.bloco(b)?;
                    blocos.push(b);
                    b = self.proximo(b);
                }
                return Ok((blocos, tam));
            }
        }
        Err("savedata.bin não encontrado no pacote".into())
    }

    pub fn le(&self) -> Result<Vec<u8>> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        let mut out = Vec::with_capacity(blocos.len() * 0x1000);
        for b in blocos {
            out.extend_from_slice(self.bloco(b)?);
        }
        out.truncate(tam);
        Ok(out)
    }

    pub fn grava(&mut self, conteudo: &[u8]) -> Result<()> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        if conteudo.len() != tam {
            return Err("o tamanho do arquivo interno não pode mudar".into());
        }
        for (i, &b) in blocos.iter().enumerate() {
            let parte = &conteudo[i * 0x1000..((i + 1) * 0x1000).min(tam)];
            let o = self.off(b);
            self.d[o..o + parte.len()].copy_from_slice(parte);
            let h = sha1(&self.d[o..o + 0x1000]);
            let e = self.entrada(b);
            self.d[e..e + 20].copy_from_slice(&h);
        }
        let topo = sha1(&self.d[self.tabela..self.tabela + 0x1000]);
        self.d[VD + 8..VD + 0x1C].copy_from_slice(&topo);
        let cab = sha1(&self.d[0x344..self.base]);
        self.d[0x32C..0x340].copy_from_slice(&cab);
        Ok(())
    }

    fn reassina(&mut self, k: &ChaveConsole) {
        self.d[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let s = k.assina(&self.d[0x22C..0x344]);
        self.d[0x1AC..0x22C].copy_from_slice(&s);
    }

    pub fn hashes_ok(&self) -> bool {
        let Ok((blocos, _)) = self.blocos_do_arquivo() else { return false };
        let blocos_ok = blocos.iter().all(|&b| {
            let e = self.entrada(b);
            self.bloco(b).map(|x| sha1(x)[..] == self.d[e..e + 20]).unwrap_or(false)
        });
        let topo = sha1(&self.d[self.tabela..self.tabela + 0x1000])[..] == self.d[VD + 8..VD + 0x1C];
        let cab = sha1(&self.d[0x344..self.base])[..] == self.d[0x32C..0x340];
        blocos_ok && topo && cab
    }
}

// ---------------------------------------------------------------- o save aberto

pub enum Plataforma {
    Xbox(Stfs),
    Pc,
}

pub struct Save {
    pub caminho: PathBuf,
    pub plat: Plataforma,
    pub interno: Interno,
}

fn slot_inv(d: &[u8], i: usize) -> Slot {
    let o = XBOX_INV + i * 12;
    Slot {
        slot: i,
        id: u16::from_be_bytes([d[o], d[o + 1]]) as u32,
        amount: u16::from_be_bytes([d[o + 2], d[o + 3]]) as u32,
    }
}

fn slots_pers(d: &[u8], base: usize) -> Vec<Slot> {
    (0..XBOX_PERS_VISIVEIS)
        .map(|i| {
            let o = base + i * XBOX_SLOT_PERS;
            Slot { slot: i, id: be32(d, o), amount: be32(d, o + 4) }
        })
        .collect()
}

impl Save {
    /// Abre e descobre a plataforma sozinho: pacote CON = Xbox 360; senão
    /// tenta o formato do PC e confere o checksum.
    pub fn abre(caminho: &Path) -> Result<Self> {
        let bruto = fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        if e_pacote_stfs(&bruto) {
            let pkg = Stfs::novo(bruto)?;
            let d = pkg.le()?;
            if d.len() < TAM_MINIMO {
                return Err("o savedata.bin do pacote é pequeno demais para ser do RE5".into());
            }
            return Ok(Self { caminho: caminho.into(), plat: Plataforma::Xbox(pkg), interno: Interno::xbox(d) });
        }
        if bruto.len() < TAM_MINIMO {
            return Err("arquivo pequeno demais para ser um save do RE5".into());
        }
        let interno = Interno::pc(pc_decifra(&bruto));
        if interno.checksum_salvo() != interno.checksum() {
            return Err("não reconheci o arquivo: não é um pacote do Xbox 360 e, decifrado como save de PC, o checksum não bate".into());
        }
        Ok(Self { caminho: caminho.into(), plat: Plataforma::Pc, interno })
    }

    pub fn info(&self) -> Value {
        let mut v = self.interno.comum_json();
        v["caminho"] = json!(self.caminho.display().to_string());
        match &self.plat {
            Plataforma::Pc => {
                v["plataforma"] = json!("pc");
                v["steam_id"] = json!(u64::from_le_bytes(self.interno.d[0..8].try_into().unwrap()).to_string());
            }
            Plataforma::Xbox(pkg) => {
                let d = &self.interno.d;
                let (perfil, device) = ids_do_pacote(&pkg.d);
                v["plataforma"] = json!("xbox");
                v["profile_id"] = json!(perfil);
                v["device_id"] = json!(device);
                v["assinatura_ok"] = json!(assinatura_ok(&pkg.d));
                v["assinado_por"] = json!(String::from_utf8_lossy(&pkg.d[0xB..0x16]));
                v["inventario"] = json!((0..XBOX_INV_SLOTS).map(|i| slot_inv(d, i)).collect::<Vec<_>>());
                v["chris"] = json!(slots_pers(d, XBOX_CHRIS));
                v["sheva"] = json!(slots_pers(d, XBOX_SHEVA));
            }
        }
        v
    }

    pub fn aplica(&mut self, m: &Mudancas) -> Result<()> {
        self.interno.aplica_comum(m)?;
        match &mut self.plat {
            Plataforma::Pc => {
                if let Some(s) = m.steam_id.as_deref().filter(|s| !s.trim().is_empty()) {
                    let id: u64 = s.trim().parse().map_err(|_| "Steam ID precisa ser um número".to_string())?;
                    self.interno.d[0..8].copy_from_slice(&id.to_le_bytes());
                }
            }
            Plataforma::Xbox(pkg) => {
                for (valor, (o, t), nome) in [
                    (&m.profile_id, PKG_PERFIL, "O perfil (XUID)"),
                    (&m.device_id, PKG_DEVICE, "O device ID"),
                ] {
                    if let Some(v) = valor.as_deref().filter(|v| !v.is_empty()) {
                        pkg.d[o..o + t].copy_from_slice(&de_hex(v, t, nome)?);
                    }
                }
                let d = &mut self.interno.d;
                for s in &m.inventario {
                    if s.slot >= XBOX_INV_SLOTS || s.id > 0xFFFF || s.amount > 0xFFFF {
                        return Err("slot do inventário inválido".into());
                    }
                    let o = XBOX_INV + s.slot * 12;
                    d[o..o + 2].copy_from_slice(&(s.id as u16).to_be_bytes());
                    d[o + 2..o + 4].copy_from_slice(&(s.amount as u16).to_be_bytes());
                }
                for (slots, base) in [(&m.chris, XBOX_CHRIS), (&m.sheva, XBOX_SHEVA)] {
                    for s in slots {
                        if s.slot >= XBOX_PERS_VISIVEIS {
                            return Err("slot de personagem inválido".into());
                        }
                        let o = base + s.slot * XBOX_SLOT_PERS;
                        d[o..o + 4].copy_from_slice(&s.id.to_be_bytes());
                        d[o + 4..o + 8].copy_from_slice(&s.amount.to_be_bytes());
                    }
                }
            }
        }
        Ok(())
    }

    /// Monta o arquivo final. No Xbox, assina se houver keyvault; devolve
    /// os bytes e o console que assinou.
    pub fn monta(&mut self, kv: Option<&Path>) -> Result<(Vec<u8>, Option<String>)> {
        self.interno.fecha_checksum();
        match &mut self.plat {
            Plataforma::Pc => Ok((pc_cifra(&self.interno.d), None)),
            Plataforma::Xbox(pkg) => {
                pkg.grava(&self.interno.d)?;
                let mut console = None;
                if let Some(kv) = kv {
                    let k = ChaveConsole::carrega(kv)?;
                    pkg.reassina(&k);
                    console = Some(k.console);
                }
                Ok((pkg.d.clone(), console))
            }
        }
    }

    /// Relê um arquivo gravado e confere tudo que dá para conferir.
    pub fn confere(caminho: &Path, assinado: bool) -> bool {
        let Ok(s) = Save::abre(caminho) else { return false };
        let chk = s.interno.checksum_salvo() == s.interno.checksum();
        match &s.plat {
            Plataforma::Pc => chk,
            Plataforma::Xbox(pkg) => chk && pkg.hashes_ok() && (!assinado || assinatura_ok(&pkg.d)),
        }
    }
}

/// Perfil e device ID de outro save do Xbox (para trocar de perfil/console).
pub fn ids_de_outro(caminho: &Path) -> Result<(String, String)> {
    let d = fs::read(caminho).map_err(|e| e.to_string())?;
    if !e_pacote_stfs(&d) {
        return Err("esse arquivo não é um save do Xbox 360 (pacote CON)".into());
    }
    Ok(ids_do_pacote(&d))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn xor_do_pc_ida_e_volta() {
        let claro: Vec<u8> = (0..0x5663u32).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(pc_decifra(&pc_cifra(&claro)), claro);
    }

    #[test]
    fn bits_preservam_o_que_nao_e_da_lista() {
        let mut i = Interno::xbox(vec![0; TAM_MINIMO]);
        let fig = &LISTAS[5];
        i.set_bits(fig, 1 << 45 | 1); // bit 45 não está na lista
        let m = Mudancas { listas: vec![("figuras".into(), vec![true; 45])], ..Default::default() };
        i.aplica_comum(&m).unwrap();
        assert_eq!(i.bits(fig), (1 << 46) - 1);
        let m = Mudancas { listas: vec![("figuras".into(), vec![false; 45])], ..Default::default() };
        i.aplica_comum(&m).unwrap();
        assert_eq!(i.bits(fig), 1 << 45);
        assert_eq!(i.u32(0x84), 0);
        assert_eq!(i.u32(0x88), 1 << 13);
    }
}
