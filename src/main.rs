use cloud_cli::error::Result;
use cloud_cli::{config_loader, run_cli, ui};
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--version" {
        println!("cloud-cli version: {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    ui::print_header();
    let doris_config = config_loader::load_config()?;
    let config = config_loader::to_app_config(doris_config);
    run_cli(config)
}
