use cloud_cli::error::Result;
use cloud_cli::{run_cli, ui};

fn main() -> Result<()> {
    ui::print_header();
    run_cli()
}
