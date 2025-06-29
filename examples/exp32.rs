use pinout::parser::csv::parse_csv_file;
use pinout::parser::types::Command;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Parse the ESP32-MAXIO.csv file
    let path = "ESP32-MAXIO.csv";
    let commands = parse_csv_file(path)?;

    println!(
        "Successfully parsed {} commands from {}",
        commands.len(),
        path
    );

    // Count commands by phase
    let setup_commands = commands
        .iter()
        .take_while(|cmd| !matches!(cmd, Command::Draw))
        .count();

    let draw_commands = commands.len() - setup_commands - 1; // -1 for the Draw command itself

    println!("Setup phase: {} commands", setup_commands);
    println!("Draw phase: {} commands", draw_commands);

    // Print some examples of parsed commands
    println!("\nExamples of parsed commands:");

    // First 3 setup commands
    println!("\n--- First 3 Setup Commands ---");
    for (i, cmd) in commands.iter().take(3).enumerate() {
        println!("{}: {:?}", i + 1, cmd);
    }

    // The first wire command
    println!("\n--- First Wire Command ---");
    if let Some(wire_cmd) = commands
        .iter()
        .find(|cmd| matches!(cmd, Command::Wire { .. }))
    {
        println!("{:?}", wire_cmd);
    }

    // The first pin command (in Draw phase)
    println!("\n--- First Pin Command ---");
    if let Some(pin_cmd) = commands
        .iter()
        .find(|cmd| matches!(cmd, Command::Pin { .. }))
    {
        println!("{:?}", pin_cmd);
    }

    // The first box command (in Draw phase)
    println!("\n--- First Box Command (Draw phase) ---");
    if let Some(box_cmd) = commands
        .iter()
        .skip_while(|cmd| !matches!(cmd, Command::Draw))
        .skip(1) // Skip the Draw command itself
        .find(|cmd| matches!(cmd, Command::Box { .. }))
    {
        println!("{:?}", box_cmd);
    }

    // Count different types of commands
    let label_count = commands
        .iter()
        .filter(|cmd| matches!(cmd, Command::Labels { .. }))
        .count();
    let pin_count = commands
        .iter()
        .filter(|cmd| matches!(cmd, Command::Pin { .. }))
        .count();
    let pintext_count = commands
        .iter()
        .filter(|cmd| matches!(cmd, Command::PinText { .. }))
        .count();
    let text_count = commands
        .iter()
        .filter(|cmd| matches!(cmd, Command::Text { .. }))
        .count();
    let box_count = commands
        .iter()
        .filter(|cmd| matches!(cmd, Command::Box { .. }))
        .count();

    println!("\n--- Command Statistics ---");
    println!("Label commands: {}", label_count);
    println!("Pin commands: {}", pin_count);
    println!("PinText commands: {}", pintext_count);
    println!("Text commands: {}", text_count);
    println!("Box commands: {}", box_count);

    Ok(())
}
