// src/main.rs
use clap::{Arg, Command};
use pinout::parser::csv::parse_csv_file;
use pinout::renderer::svg::generate_svg;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("GenPinout SVG")
        .version("1.0")
        .author("Rust Version")
        .about("Generates pinout diagrams in SVG format from CSV descriptions")
        .arg(
            Arg::new("csv_file")
                .help("Input CSV file with pinout description")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("svg_file")
                .help("Output SVG file (defaults to csv filename with .svg extension)")
                .index(2),
        )
        .arg(
            Arg::new("overwrite")
                .help("Overwrite existing SVG file if it exists")
                .long("overwrite")
                .short('o')
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let csv_path = matches.get_one::<String>("csv_file").unwrap();

    // Determine SVG output path
    let svg_path = match matches.get_one::<String>("svg_file") {
        Some(path) => path.clone(),
        None => {
            let csv_file = Path::new(csv_path);
            let stem = csv_file.file_stem().unwrap().to_str().unwrap();
            format!("svg/{}.svg", stem)
        }
    };

    // Create directory for SVG if it doesn't exist
    if let Some(parent) = Path::new(&svg_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Check if SVG file exists and if we're allowed to overwrite
    if Path::new(&svg_path).exists() && !matches.get_flag("overwrite") {
        return Err(format!(
            "SVG file {} exists. Use --overwrite to overwrite.",
            svg_path
        )
        .into());
    }

    // Parse the CSV file
    let commands = parse_csv_file(csv_path)?;

    // Generate the SVG from the commands
    generate_svg(&commands, &svg_path)?;

    println!("Successfully generated SVG: {}", svg_path);

    Ok(())
}
