// Compila a interface Slint e, no Windows, grava o ícone e o nome no .exe.
fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("material".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("erro na interface Slint");

    // Versão pessoal: copia o kv.bin para a pasta de compilação (fora do
    // repositório); o programa o embute com include_bytes!.
    println!("cargo:rerun-if-env-changed=RE5_KV_EMBUTIDO");
    let pessoal = std::env::var_os("CARGO_FEATURE_KV_EMBUTIDO").is_some();
    if pessoal {
        let origem = std::env::var("RE5_KV_EMBUTIDO")
            .expect("versão pessoal: defina RE5_KV_EMBUTIDO com o caminho do kv.bin");
        println!("cargo:rerun-if-changed={origem}");
        let kv = std::fs::read(&origem).expect("não consegui ler o kv.bin");
        assert!(kv.len() >= 0x4000, "kv.bin pequeno demais");
        let destino = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("kv_embutido.bin");
        std::fs::write(destino, kv).unwrap();
    }
    let nome = if pessoal { "RE5 EDITOR PESSOAL GOLD" } else { "RE5 Save Editor 360+PC" };

    println!("cargo:rerun-if-changed=ui/imagens/icone.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/imagens/icone.ico")
            .set("ProductName", nome)
            .set("FileDescription", nome);
        if let Err(e) = res.compile() {
            println!("cargo:warning=icone do .exe nao embutido: {e}");
        }
    }
}
