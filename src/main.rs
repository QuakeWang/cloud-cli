use cloud_cli::error::Result;
use cloud_cli::{config_loader, run_cli, ui};

fn main() -> Result<()> {
    ui::print_header();
    let doris_config = config_loader::load_config()?;
    let config = config_loader::to_app_config(doris_config);
    run_cli(config)
}
