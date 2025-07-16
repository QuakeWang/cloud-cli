use cloud_cli::error::Result;
use cloud_cli::{config::Config, run_cli, ui};

fn main() -> Result<()> {
    ui::print_header();

    let config = Config::new();

    run_cli(config)
}
