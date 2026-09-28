//! Pinout diagrams for microcontrollers, development boards and chips.
//!
//! Boards are described in the YAML format of
//! [pinoutleaf](https://github.com/splitbrain/pinoutleaf) and read into a
//! [`model::Pinout`]. The renderers draw it as a true-to-scale SVG
//! ([`renderer::leaf`]) or as colored terminal text ([`renderer::term`]), and
//! serde writes it back as YAML or JSON.
//!
//! ```
//! use pinout::model::Pinout;
//! use pinout::renderer::term::{render_terminal, TermOptions};
//!
//! let pinout = Pinout::from_yaml_str(
//!     r#"
//! title: Demo
//! pins:
//!   left:
//!     - [ "GPIO5:gpio", "MISO:spi" ]
//!   right:
//!     - [ "GND:gnd" ]
//! "#,
//! )?;
//! let options = TermOptions {
//!     color: false,
//!     ..TermOptions::default()
//! };
//! assert!(render_terminal(&pinout, &options)?.contains("MISO"));
//! # Ok::<(), pinout::model::PinoutError>(())
//! ```
//!
//! The legacy CSV descriptions are read by [`parser`], drawn by
//! [`renderer::svg`] and converted to the model by [`import`].
//!
//! The default `cli` feature builds the `pinout` command line tool and
//! terminal UI; library users can turn it off with
//! `default-features = false`.

pub mod import;
pub mod model;
pub mod parser;
pub mod renderer;
pub use parser::csv;
pub use parser::document;
pub use parser::types;
pub use renderer::svg;
pub use renderer::term;
