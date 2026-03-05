use anyhow::Result;
use serde_json::json;

use crate::application::dto::DoctorInput;
use crate::application::ports::HealthRepository;
use crate::application::usecases::doctor_report;
use crate::config::AppPaths;
use crate::output::{Output, OutputMode};

pub fn run_doctor<R>(
    paths: &AppPaths,
    repo: &R,
    mode: OutputMode,
    color_enabled: bool,
) -> Result<()>
where
    R: HealthRepository,
{
    let report = doctor_report(
        repo,
        DoctorInput {
            home: paths.home.clone(),
            config_path: paths.config_path.clone(),
            db_path: paths.db_path.clone(),
        },
    )?;

    let out = Output::new(mode, color_enabled);
    if mode == OutputMode::Json {
        out.json(&json!({
            "home": report.home,
            "config_path": report.config_path,
            "db_path": report.db_path,
            "config_exists": report.config_exists,
            "db_exists": report.db_exists,
            "bookmark_count": report.bookmark_count,
            "quran_source": "API (equran.id/v2)",
            "recommended_terminal_font": "Amiri Quran",
        }))?;
        return Ok(());
    }

    out.title("tada-rust doctor");
    out.kv("home path", report.home.display().to_string());
    out.kv(
        "config.toml",
        format!(
            "{} ({})",
            report.config_path.display(),
            if report.config_exists {
                "ok"
            } else {
                "missing"
            }
        ),
    );
    out.kv(
        "database",
        format!(
            "{} ({})",
            report.db_path.display(),
            if report.db_exists { "ok" } else { "missing" }
        ),
    );
    out.kv("bookmarks", report.bookmark_count.to_string());
    out.kv("quran source", "API (equran.id/v2)");
    out.kv("font arabic", "Amiri Quran (set di terminal profile)");

    Ok(())
}
