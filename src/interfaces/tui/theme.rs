use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(deserialize_with = "deserialize_color")]
    pub bg: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub fg: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub highlight_bg: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub highlight_fg: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub accent: Color,
}

#[derive(Debug, Deserialize)]
struct ThemeConfig {
    themes: Vec<Theme>,
}

fn deserialize_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    parse_color(&s).map_err(serde::de::Error::custom)
}

fn parse_color(s: &str) -> Result<Color, String> {
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" => Ok(Color::Gray),
        "dark_gray" => Ok(Color::DarkGray),
        "light_red" => Ok(Color::LightRed),
        "light_green" => Ok(Color::LightGreen),
        "light_yellow" => Ok(Color::LightYellow),
        "light_blue" => Ok(Color::LightBlue),
        "light_magenta" => Ok(Color::LightMagenta),
        "light_cyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        _ => {
            if s.starts_with('#') {
                let hex = s.trim_start_matches('#');
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex")?;
                    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex")?;
                    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex")?;
                    Ok(Color::Rgb(r, g, b))
                } else {
                    Err("Invalid hex length".to_string())
                }
            } else {
                Err(format!("Unknown color: {}", s))
            }
        }
    }
}

pub fn load_themes<P: AsRef<Path>>(path: P) -> Result<Vec<Theme>> {
    let content = fs::read_to_string(path).context("Failed to read theme file")?;
    let config: ThemeConfig =
        serde_yaml::from_str(&content).context("Failed to parse theme file")?;
    if config.themes.is_empty() {
        return Err(anyhow::anyhow!("No themes found in configuration"));
    }
    Ok(config.themes)
}

pub fn default_themes() -> Vec<Theme> {
    vec![
        Theme {
            name: "Light".to_string(),
            bg: Color::White,
            fg: Color::Black,
            highlight_bg: Color::Blue,
            highlight_fg: Color::White,
            accent: Color::Blue,
        },
        Theme {
            name: "Dark".to_string(),
            bg: Color::Rgb(18, 18, 18),
            fg: Color::Rgb(224, 224, 224),
            highlight_bg: Color::Cyan,
            highlight_fg: Color::Black,
            accent: Color::Cyan,
        },
        // Add other defaults if needed as fallback, but for now minimal set is fine
    ]
}
