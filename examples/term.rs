use clap::{Arg, Command};
use pinout::parser::csv::parse_csv_file;
use pinout::renderer::term::{render_terminal, TermOptions};
use std::io::IsTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("GenPinout Terminal")
        .version("1.0")
        .about("Draws pinout diagrams in the terminal from CSV descriptions")
        .arg(
            Arg::new("csv_file")
                .help("Input CSV file with pinout description")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("labels")
                .help("Comma separated label columns to show (default: all)")
                .long("labels")
                .short('l'),
        )
        .arg(
            Arg::new("width")
                .help("Maximum width in columns (default: terminal width)")
                .long("width")
                .short('w')
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("compact")
                .help("No gap between pins")
                .long("compact")
                .short('c')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_color")
                .help("Disable colors")
                .long("no-color")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let csv_path = matches.get_one::<String>("csv_file").unwrap();
    let stdout = std::io::stdout();

    let options = TermOptions {
        color: !matches.get_flag("no_color")
            && std::env::var_os("NO_COLOR").is_none()
            && (stdout.is_terminal() || std::env::var_os("CLICOLOR_FORCE").is_some()),
        labels: matches
            .get_one::<String>("labels")
            .map(|l| l.split(',').map(|s| s.trim().to_string()).collect()),
        max_width: matches.get_one::<usize>("width").copied().or_else(|| {
            terminal_size::terminal_size().map(|(terminal_size::Width(w), _)| w as usize)
        }),
        title: None,
        compact: matches.get_flag("compact"),
    };

    let commands = parse_csv_file(csv_path)?;
    print!("{}", render_terminal(&commands, &options)?);

    Ok(())
}
