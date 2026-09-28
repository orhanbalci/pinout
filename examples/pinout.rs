use clap::{Arg, ArgAction, Command};
use pinout::import::from_csv_file;
use pinout::model::Pinout;
use pinout::renderer::leaf::{render_svg, SvgOptions};
use pinout::renderer::term::{render_terminal, TermOptions};
use std::io::IsTerminal;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("pinout")
        .version("1.0")
        .about("Renders pinout diagrams from YAML or JSON board descriptions")
        .arg(
            Arg::new("input")
                .help("Board description (.yaml, .yml, .json, or legacy .csv to convert)")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("format")
                .help("Output format (default: from the output extension, else term)")
                .long("format")
                .short('f')
                .value_parser(["term", "svg", "json", "yaml"]),
        )
        .arg(
            Arg::new("output")
                .help("Output file (default: stdout)")
                .long("output")
                .short('o'),
        )
        .arg(
            Arg::new("back")
                .help("Render the back of the board")
                .long("back")
                .short('b')
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("types")
                .help("Terminal: comma separated label types to show")
                .long("types")
                .short('t'),
        )
        .arg(
            Arg::new("compact")
                .help("Terminal: no gap between pins")
                .long("compact")
                .short('c')
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("width")
                .help("Terminal: maximum width in columns (default: terminal width)")
                .long("width")
                .short('w')
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("notes")
                .help("Terminal: print the board's notes (features, warnings, ...)")
                .long("notes")
                .short('n')
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("packed")
                .help("Terminal: flow labels next to each other instead of aligning columns")
                .long("packed")
                .short('p')
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("max_label")
                .help("Terminal: shorten labels longer than this; they are also shortened to fit the width")
                .long("max-label")
                .short('m')
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("no_color")
                .help("Terminal: disable colors")
                .long("no-color")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let input = Path::new(matches.get_one::<String>("input").unwrap());
    let output = matches.get_one::<String>("output").map(Path::new);

    let pinout = if extension(input) == "csv" {
        let import = from_csv_file(input)?;
        for warning in &import.warnings {
            eprintln!("warning: {warning}");
        }
        import.pinout
    } else {
        Pinout::from_path(input)?
    };

    let format = match matches.get_one::<String>("format") {
        Some(format) => format.clone(),
        None => match output.map(extension).as_deref() {
            Some("svg") => "svg".to_string(),
            Some("json") => "json".to_string(),
            Some("yaml" | "yml") => "yaml".to_string(),
            _ => "term".to_string(),
        },
    };
    let back = matches.get_flag("back");

    let rendered = match format.as_str() {
        "svg" => render_svg(
            &pinout,
            &SvgOptions {
                back,
                base_dir: input.parent().map(Path::to_path_buf),
            },
        )?,
        "json" => pinout.to_json()?,
        "yaml" => pinout.to_yaml()?,
        _ => {
            let stdout = std::io::stdout();
            let to_terminal = output.is_none() && stdout.is_terminal();
            render_terminal(
                &pinout,
                &TermOptions {
                    color: !matches.get_flag("no_color")
                        && std::env::var_os("NO_COLOR").is_none()
                        && (to_terminal || std::env::var_os("CLICOLOR_FORCE").is_some()),
                    max_width: matches.get_one::<usize>("width").copied().or_else(|| {
                        terminal_size::terminal_size()
                            .map(|(terminal_size::Width(w), _)| w as usize)
                    }),
                    compact: matches.get_flag("compact"),
                    back,
                    types: matches
                        .get_one::<String>("types")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect()),
                    notes: matches.get_flag("notes"),
                    packed: matches.get_flag("packed"),
                    max_label: matches.get_one::<usize>("max_label").copied(),
                },
            )?
        }
    };

    match output {
        Some(path) => {
            std::fs::write(path, rendered)?;
            eprintln!("{} written", path.display());
        }
        None => print!("{rendered}"),
    }
    Ok(())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
