use anyhow::{Result, anyhow};
use comfy_table::presets::{NOTHING, UTF8_BORDERS_ONLY};
use comfy_table::{ContentArrangement, Row, Table};
use owo_colors::OwoColorize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Rich,
    Plain,
    Json,
}

impl OutputMode {
    pub fn from_flags_and_config(plain: bool, json: bool, config_mode: &str) -> Result<Self> {
        if plain && json {
            return Err(anyhow!(
                "Gunakan salah satu: --plain atau --json, tidak keduanya."
            ));
        }
        if json {
            return Ok(Self::Json);
        }
        if plain {
            return Ok(Self::Plain);
        }
        match config_mode {
            "plain" => Ok(Self::Plain),
            "json" => Ok(Self::Json),
            _ => Ok(Self::Rich),
        }
    }
}

pub struct Output {
    mode: OutputMode,
    color_enabled: bool,
}

impl Output {
    pub fn new(mode: OutputMode, color_enabled: bool) -> Self {
        Self {
            mode,
            color_enabled,
        }
    }

    pub fn mode(&self) -> OutputMode {
        self.mode
    }

    pub fn json(&self, value: &Value) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    }

    pub fn title(&self, text: &str) {
        match self.mode {
            OutputMode::Rich => {
                println!();
                if self.color_enabled {
                    println!("{}", text.bold().bright_blue());
                } else {
                    println!("{text}");
                }
            }
            _ => println!("{text}"),
        }
    }

    pub fn subtitle(&self, text: &str) {
        match self.mode {
            OutputMode::Rich if self.color_enabled => println!("{}", text.bright_black()),
            _ => println!("{text}"),
        }
    }

    pub fn line(&self, text: impl AsRef<str>) {
        println!("{}", text.as_ref());
    }

    pub fn kv(&self, key: &str, value: impl AsRef<str>) {
        let value = value.as_ref();
        match (self.mode, self.color_enabled) {
            (OutputMode::Rich, true) => println!(
                "  {:<16} {}",
                key.bright_black().bold(),
                value.bright_white()
            ),
            (OutputMode::Rich, false) => println!("  {:<16} {}", key, value),
            _ => println!("{key}: {value}"),
        }
    }

    pub fn status(&self, kind: &str, msg: impl AsRef<str>) {
        let msg = msg.as_ref();
        match (self.mode, self.color_enabled) {
            (OutputMode::Rich, true) => {
                let label = match kind {
                    "OK" => kind.green().bold().to_string(),
                    "WARN" => kind.yellow().bold().to_string(),
                    "ERR" => kind.red().bold().to_string(),
                    _ => kind.cyan().bold().to_string(),
                };
                println!("[{label}] {msg}");
            }
            (OutputMode::Rich, false) => println!("[{kind}] {msg}"),
            _ => println!("{kind}: {msg}"),
        }
    }

    pub fn hint(&self, msg: impl AsRef<str>) {
        let msg = msg.as_ref();
        match (self.mode, self.color_enabled) {
            (OutputMode::Rich, true) => println!("{} {}", "Hint:".bright_blue().bold(), msg),
            _ => println!("Hint: {msg}"),
        }
    }

    pub fn table(&self, headers: &[&str], rows: &[Vec<String>]) {
        if rows.is_empty() {
            self.status("INFO", "Tidak ada data.");
            return;
        }

        if self.mode != OutputMode::Rich {
            println!("{}", headers.join("\t"));
            for row in rows {
                println!("{}", row.join("\t"));
            }
            return;
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_BORDERS_ONLY)
            .set_content_arrangement(ContentArrangement::Dynamic);
        let styled_headers: Vec<String> = headers
            .iter()
            .map(|h| {
                if self.color_enabled {
                    h.bold().bright_blue().to_string()
                } else {
                    h.to_string()
                }
            })
            .collect();
        table.set_header(Row::from(styled_headers));
        for row in rows {
            table.add_row(row.clone());
        }
        println!("{table}");
    }

    pub fn divider(&self) {
        if self.mode == OutputMode::Rich {
            let mut table = Table::new();
            table.load_preset(NOTHING);
            table.add_row(vec![
                "------------------------------------------------------------",
            ]);
            println!("{table}");
        }
    }
}
