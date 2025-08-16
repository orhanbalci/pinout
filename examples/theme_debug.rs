// Example showing how to use theme debugging functions
use pinout::parser::csv::parse_csv_file;
use pinout::renderer::svg::{SvgRenderer, generate_svg_with_debug};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let csv_file = if args.len() > 1 {
        &args[1]
    } else {
        "ESP32-MAXIO.csv" // Default file
    };

    println!("Parsing CSV file: {}", csv_file);

    // Parse the CSV file
    let commands = parse_csv_file(csv_file)?;

    println!("Found {} commands", commands.len());

    // Create a renderer and process commands
    let mut renderer = SvgRenderer::new();
    renderer.process_commands(&commands)?;

    // Print all themes
    renderer.print_themes();

    // Print specific themes if they exist
    renderer.print_theme("DEFAULT");
    renderer.print_theme("BOX_STD");
    renderer.print_theme("FONT_DEFAULT");

    // Generate SVG with theme debugging enabled
    generate_svg_with_debug(&commands, "output_debug.svg", true)?;

    println!("SVG generated successfully!");

    Ok(())
}
