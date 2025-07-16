use console::{Term, style};

pub mod menu;

pub use menu::*;

pub static SUCCESS: &str = "[+] ";
pub static ERROR: &str = "[!] ";
pub static WARNING: &str = "[*] ";
pub static INFO: &str = "[i] ";
pub static PROCESS: &str = "[>] ";
pub static SEARCH: &str = "[?] ";

enum MessageType {
    Success,
    Error,
    Warning,
    Info,
}

fn print_message(level: MessageType, message: &str) {
    match level {
        MessageType::Success => {
            println!("{}", style(format!("{SUCCESS} {message}")).green().bold())
        }
        MessageType::Error => eprintln!("{}", style(format!("{ERROR} {message}")).red().bold()),
        MessageType::Warning => {
            println!("{}", style(format!("{WARNING} {message}")).yellow().bold())
        }
        MessageType::Info => println!("{}", style(format!("{INFO} {message}")).blue()),
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_header() {
    let term = Term::stdout();
    // Fallback width if terminal size can't be determined
    let width = term.size_checked().map(|s| s.1 as usize).unwrap_or(80);

    let title = "SelectDB CLI Tools for Apache Doris";
    let version_info = format!("Version {VERSION}");

    println!();
    println!("{}", style("─".repeat(width)).dim());
    println!("{:^width$}", style(title).cyan().bold());
    println!("{:^width$}", style(version_info).dim());
    println!("{}", style("─".repeat(width)).dim());
    println!();
}

pub fn print_success(message: &str) {
    print_message(MessageType::Success, message);
}

pub fn print_error(message: &str) {
    print_message(MessageType::Error, message);
}

pub fn print_warning(message: &str) {
    print_message(MessageType::Warning, message);
}

pub fn print_info(message: &str) {
    print_message(MessageType::Info, message);
}

pub fn print_step(step: u8, message: &str) {
    println!();
    println!(
        "{} {}",
        style(format!("Step {step}")).cyan().bold(),
        style(message).bold()
    );
}

pub fn print_process_info(pid: u32, command: &str) {
    println!();
    println!("{PROCESS} Process Details:");
    println!(
        "  {} PID: {}",
        style(">").blue(),
        style(pid.to_string()).green().bold()
    );
    println!(
        "  {} Command: {}",
        style(">").blue(),
        style(truncate_command(command, 60)).dim()
    );
}

pub fn print_goodbye() {
    println!();
    println!(
        "{}",
        style("Thanks for using SelectDB Cloud CLI Tools!")
            .green()
            .bold()
    );
    println!();
}

pub fn truncate_command(command: &str, max_len: usize) -> String {
    if command.len() <= max_len {
        command.to_string()
    } else {
        format!("{}...", &command[..max_len])
    }
}

pub fn format_menu_item(icon: &str, title: &str, description: &str) -> String {
    format!(
        "{} {} - {}",
        style(icon).blue(),
        style(title).bold(),
        style(description).dim()
    )
}
