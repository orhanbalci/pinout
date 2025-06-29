use super::{
    csv::{ParserError, parse_csv_file},
    types::{Command, Phase},
};

pub struct Document {
    pub commands: Vec<Command>,
    pub phase: Phase,
}

impl Document {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            phase: Phase::Setup,
        }
    }

    pub fn from_file(path: &str) -> Result<Self, ParserError> {
        let commands = parse_csv_file(path)?;
        let phase = if commands.iter().any(|cmd| matches!(cmd, Command::Draw)) {
            Phase::Draw
        } else {
            Phase::Setup
        };

        Ok(Self { commands, phase })
    }

    pub fn add_command(&mut self, command: Command) -> Result<(), ParserError> {
        // Check if command is valid for current phase
        match (&command, self.phase) {
            (Command::Draw, _) => {
                self.phase = Phase::Draw;
            }
            (_, Phase::Setup) if is_setup_command(&command) => {}
            (_, Phase::Draw) if is_draw_command(&command) => {}
            _ => return Err(ParserError::InvalidPhase),
        }

        self.commands.push(command);
        Ok(())
    }
}

fn is_setup_command(command: &Command) -> bool {
    matches!(
        command,
        Command::Label { .. }
            | Command::BorderColor { .. }
            | Command::FillColor { .. }
            | Command::Opacity { .. }
            | Command::Font { .. }
            | Command::FontSize { .. }
            | Command::FontColor { .. }
            | Command::FontSlant { .. }
            | Command::FontBold { .. }
            | Command::FontStretch { .. }
            | Command::Type { .. }
            | Command::Wire { .. }
            | Command::Group { .. }
            | Command::BoxTheme { .. }
            | Command::TextFont { .. }
            | Command::Page { .. }
            | Command::Dpi { .. }
    )
}

fn is_draw_command(command: &Command) -> bool {
    matches!(
        command,
        Command::GoogleFont { .. }
            | Command::Image { .. }
            | Command::Icon { .. }
            | Command::Anchor { .. }
            | Command::PinSet { .. }
            | Command::Pin { .. }
            | Command::PinText { .. }
            | Command::Box { .. }
            | Command::Message { .. }
            | Command::Text { .. }
            | Command::EndMessage
    )
}
