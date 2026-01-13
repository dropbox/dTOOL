//! Example: Discover all DashFlow modules

use dashflow_module_discovery::discover_modules;

fn main() {
    let modules = discover_modules("crates/dashflow/src");

    println!("=== Discovered {} modules ===\n", modules.len());

    for module in &modules {
        println!("📦 {}", module.path);
        println!("   Category: {}", module.category);
        if !module.description.is_empty() {
            let desc = if module.description.len() > 60 {
                format!("{}...", &module.description[..57])
            } else {
                module.description.clone()
            };
            println!("   {}", desc);
        }
        if let Some(cli) = &module.cli_command {
            println!("   CLI: {} ({:?})", cli, module.cli_status);
        }
        println!();
    }
}
