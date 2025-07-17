// src/main.rs
use clap::{App, Arg};
use genpinout::parser::csv::parse_csv_file;
use genpinout::renderer::svg::generate_svg;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("GenPinout SVG")
        .version("1.0")
        .author("Rust Version")
        .about("Generates pinout diagrams in SVG format from CSV descriptions")
        .arg(
            Arg::with_name("csv_file")
                .help("Input CSV file with pinout description")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("svg_file")
                .help("Output SVG file (defaults to csv filename with .svg extension)")
                .index(2),
        )
        .arg(
            Arg::with_name("overwrite")
                .help("Overwrite existing SVG file if it exists")
                .long("overwrite")
                .short("o")
                .takes_value(false),
        )
        .get_matches();

    let csv_path = matches.value_of("csv_file").unwrap();

    // Determine SVG output path
    let svg_path = match matches.value_of("svg_file") {
        Some(path) => path.to_string(),
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
    if Path::new(&svg_path).exists() && !matches.is_present("overwrite") {
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
