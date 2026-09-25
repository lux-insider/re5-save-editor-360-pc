// No Windows, grava o ícone e o nome do programa dentro do .exe.
fn main() {
    println!("cargo:rerun-if-changed=ui/icone.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/icone.ico")
            .set("ProductName", "RE5 Save Editor 360+PC")
            .set("FileDescription", "RE5 Save Editor 360+PC");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icone do .exe nao embutido: {e}");
        }
    }
}
