// Compila a interface Slint e, no Windows, grava o ícone e o nome no .exe.
fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("material".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("erro na interface Slint");

    println!("cargo:rerun-if-changed=ui/imagens/icone.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/imagens/icone.ico")
            .set("ProductName", "RE5 Save Editor 360+PC")
            .set("FileDescription", "RE5 Save Editor 360+PC");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icone do .exe nao embutido: {e}");
        }
    }
}
