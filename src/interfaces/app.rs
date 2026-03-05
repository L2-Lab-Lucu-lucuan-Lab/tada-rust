use std::io::IsTerminal;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Confirm;
use serde_json::{Value, json};

use crate::application::dto::{
    BookmarkAddInput, ContinueInput, PlanInput, PlayInput, ReadInput, ReadOutput, ReadTarget,
    SearchInput, SearchScope as AppSearchScope, SyncInput, SyncOutput, SyncStatus,
};
use crate::application::usecases::{
    add_bookmark, continue_reading, daily_plan, list_bookmarks, prepare_play, read_quran,
    remove_bookmark, search_quran, sync_content,
};
use crate::cli::{
    BookmarkCommand, Command, ConfigCommand, PlayArgs, ReadArgs, SearchArgs, SearchScope,
};
use crate::config::{AppConfig, AppPaths, ensure_notice, load_or_create, resolve_paths, save};
use crate::doctor::run_doctor;
use crate::domain::{
    AyahNumber, AyahRef, BookmarkId, LanguageTag, PlanCount, QariId, SearchLimit, SurahNumber,
};
use crate::interactive::{WizardDecision, prompt_command};
use crate::output::{Output, OutputMode};
use crate::quran_api::QuranApiClient;
use crate::storage::Database;
use crate::tui::{TuiLaunchOptions, run_tui};

pub struct AppContext {
    pub paths: AppPaths,
    pub config: AppConfig,
    pub db: Database,
    pub api: QuranApiClient,
    pub output_mode: OutputMode,
    pub color_enabled: bool,
    pub assume_yes: bool,
}

impl AppContext {
    pub fn bootstrap(data_dir: Option<&Path>) -> Result<Self> {
        let paths = resolve_paths(data_dir)?;
        let config = load_or_create(&paths)?;
        ensure_notice(&paths)?;

        let db = Database::open(&paths.db_path)?;
        let api = QuranApiClient::new()?;

        Ok(Self {
            paths,
            config,
            db,
            api,
            output_mode: OutputMode::Rich,
            color_enabled: true,
            assume_yes: false,
        })
    }
}

pub fn execute_command(ctx: &mut AppContext, command: Command) -> Result<()> {
    match command {
        Command::Interactive => run_interactive(ctx)?,
        Command::Read(args) => run_read(ctx, args)?,
        Command::Play(args) => run_play(ctx, args)?,
        Command::Search(args) => run_search(ctx, args)?,
        Command::Bookmark { command } => run_bookmark(ctx, command)?,
        Command::Continue => run_continue(ctx)?,
        Command::Plan { count } => run_plan(ctx, count)?,
        Command::Sync { force } => run_sync(ctx, force)?,
        Command::Tui => {
            let options = TuiLaunchOptions {
                theme_mode: ctx.config.ui_theme().to_string(),
                show_translation: ctx.config.show_translation,
                audio_cache_root: ctx.paths.home.join("cache").join("audio"),
                default_qari: ctx.config.audio.default_qari.clone(),
                cache_enabled: ctx.config.audio.cache_enabled,
                cache_max_mb: ctx.config.audio.cache_max_mb,
                initial_surah: None,
                initial_ayah: None,
                autoplay: false,
                qari_override: None,
                color_enabled: ctx.color_enabled,
            };
            run_tui(&ctx.api, &ctx.db, &ctx.config.default_lang, &options)?;
        }
        Command::Config { command } => run_config(ctx, command)?,
        Command::Doctor => run_doctor(&ctx.paths, &ctx.db, ctx.output_mode, ctx.color_enabled)?,
    }
    Ok(())
}

fn run_interactive(ctx: &mut AppContext) -> Result<()> {
    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    out.title("Interactive Mode");
    out.subtitle("Pilih aksi dari wizard. Tekan Ctrl+C untuk keluar.");

    loop {
        let (decision, command) = prompt_command(&ctx.config.default_lang, ctx.assume_yes)?;
        if decision == WizardDecision::Exit {
            out.status("INFO", "Sesi interactive selesai.");
            break;
        }
        if let Some(cmd) = command {
            execute_command(ctx, cmd)?;
            out.divider();
        }
    }
    Ok(())
}

fn run_read(ctx: &mut AppContext, args: ReadArgs) -> Result<()> {
    let input = build_read_input(args.surah, args.ayah, &args.lang)?;
    let output = with_spinner(ctx.output_mode, "Mengambil data ayat...", || {
        read_quran(&ctx.api, &ctx.db, input)
            .context("Gagal membaca ayat. Cek koneksi internet dan ulangi perintah.")
    })?;

    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    if out.mode() == OutputMode::Json {
        match output {
            ReadOutput::Single(ayah) => {
                out.json(&json!({"kind": "single", "ayah": ayah_to_json(&ayah)}))?
            }
            ReadOutput::Surah(ayahs) => out.json(&json!({
                "kind": "surah",
                "ayahs": ayahs.iter().map(ayah_to_json).collect::<Vec<_>>()
            }))?,
        }
        return Ok(());
    }

    match output {
        ReadOutput::Single(ayah) => {
            out.title("Read");
            out.kv("Ayat", format!("{}:{}", ayah.surah_no, ayah.ayah_no));
            out.line(ayah.arabic_text);
            out.line(ayah.transliteration.unwrap_or_else(|| "-".to_string()));
            out.line(ayah.translation.unwrap_or_else(|| "-".to_string()));
            out.kv("Audio", ayah.audio_url.unwrap_or_else(|| "-".to_string()));
        }
        ReadOutput::Surah(ayahs) => {
            out.title("Read Surah");
            out.subtitle("Gunakan `play --surah <N>` untuk mode audio/TUI.");
            for ayah in ayahs {
                out.line(format!(
                    "{}:{} {}",
                    ayah.surah_no, ayah.ayah_no, ayah.arabic_text
                ));
                if ctx.config.show_translation {
                    out.line(format!(
                        "    {}",
                        ayah.translation.as_deref().unwrap_or("-")
                    ));
                }
                if let Some(audio) = ayah.audio_url.as_deref() {
                    out.line(format!("    [audio] {audio}"));
                }
            }
        }
    }

    Ok(())
}

fn run_play(ctx: &mut AppContext, args: PlayArgs) -> Result<()> {
    let requested_qari = args
        .qari
        .as_deref()
        .map(QariId::new)
        .transpose()
        .with_context(|| {
            format!(
                "Qari tidak valid: {}. Gunakan 01..06",
                args.qari.as_deref().unwrap_or_default()
            )
        })?;
    let target = build_read_target(args.surah, args.ayah)?;
    let fallback_qari = QariId::new(&ctx.config.audio.default_qari)
        .context("Config qari default tidak valid. Set audio.default_qari ke 01..06")?;
    let lang = LanguageTag::new(&ctx.config.default_lang)
        .context("default_lang config tidak valid untuk command play")?;

    let output = with_spinner(ctx.output_mode, "Menyiapkan audio...", || {
        prepare_play(
            &ctx.api,
            PlayInput {
                target,
                lang,
                qari: requested_qari.clone(),
                fallback_qari,
            },
        )
        .context("Gagal menyiapkan audio. Cek koneksi internet atau ganti qari.")
    })?;

    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    if out.mode() == OutputMode::Json {
        out.json(&json!({
            "surah": args.surah,
            "ayah": args.ayah,
            "qari": output.selected_qari.as_ref().map(QariId::as_str),
            "target_audio": output.target_audio,
            "items": output.ayahs.iter().map(ayah_to_json).collect::<Vec<_>>()
        }))?;
        return Ok(());
    }

    if output.ayahs.is_empty() {
        out.status("WARN", "Ayat tidak ditemukan.");
        return Ok(());
    }

    if args.open {
        if let Some(url) = output.target_audio {
            webbrowser::open(&url).context("Gagal membuka audio di browser/player default")?;
            out.status("OK", "Audio dibuka di player/browser default.");
        } else {
            out.status("WARN", "Audio target tidak tersedia.");
        }
        return Ok(());
    }

    let options = TuiLaunchOptions {
        theme_mode: ctx.config.ui_theme().to_string(),
        show_translation: ctx.config.show_translation,
        audio_cache_root: ctx.paths.home.join("cache").join("audio"),
        default_qari: ctx.config.audio.default_qari.clone(),
        cache_enabled: ctx.config.audio.cache_enabled && !args.no_cache,
        cache_max_mb: ctx.config.audio.cache_max_mb,
        initial_surah: Some(args.surah),
        initial_ayah: args.ayah,
        autoplay: true,
        qari_override: requested_qari.map(|id| id.as_str().to_string()),
        color_enabled: ctx.color_enabled,
    };
    run_tui(&ctx.api, &ctx.db, &ctx.config.default_lang, &options)
}

fn run_search(ctx: &mut AppContext, args: SearchArgs) -> Result<()> {
    let scope = match args.scope {
        SearchScope::Quran => AppSearchScope::Quran,
        SearchScope::Translation => AppSearchScope::Translation,
        SearchScope::All => AppSearchScope::All,
    };
    let limit = SearchLimit::new(args.limit).context("Limit search tidak valid (1..200)")?;
    let output = with_spinner(ctx.output_mode, "Mencari ayat...", || {
        search_quran(
            &ctx.api,
            SearchInput {
                query: args.query.clone(),
                scope,
                limit,
            },
        )
        .context("Pencarian gagal. Periksa koneksi internet lalu coba lagi.")
    })?;

    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    if out.mode() == OutputMode::Json {
        out.json(&json!({
            "query": args.query,
            "hits": output.hits.iter().map(|h| {
                json!({"surah_no": h.surah_no, "ayah_no": h.ayah_no, "snippet": h.snippet})
            }).collect::<Vec<_>>()
        }))?;
        return Ok(());
    }

    if output.hits.is_empty() {
        out.status(
            "INFO",
            format!("Tidak ada hasil untuk query: {}", args.query),
        );
        out.hint("Coba ganti kata kunci atau set --scope all.");
        return Ok(());
    }

    out.title("Search Results");
    let rows: Vec<Vec<String>> = output
        .hits
        .iter()
        .enumerate()
        .map(|(idx, hit)| {
            vec![
                (idx + 1).to_string(),
                hit.surah_no.to_string(),
                hit.ayah_no.to_string(),
                hit.snippet.clone(),
            ]
        })
        .collect();
    out.table(&["No", "Surah", "Ayah", "Snippet"], &rows);
    Ok(())
}

fn run_bookmark(ctx: &mut AppContext, cmd: BookmarkCommand) -> Result<()> {
    let out = Output::new(ctx.output_mode, ctx.color_enabled);

    match cmd {
        BookmarkCommand::Add(args) => {
            let target = AyahRef::new(SurahNumber::new(args.surah)?, AyahNumber::new(args.ayah)?);
            let id = add_bookmark(
                &ctx.db,
                BookmarkAddInput {
                    target,
                    note: args.note.clone(),
                },
            )?;
            if out.mode() == OutputMode::Json {
                out.json(&json!({
                    "id": id.value(),
                    "surah": args.surah,
                    "ayah": args.ayah,
                    "note": args.note
                }))?;
            } else {
                out.status(
                    "OK",
                    format!(
                        "Bookmark #{} ditambahkan di {}:{}",
                        id.value(),
                        args.surah,
                        args.ayah
                    ),
                );
            }
        }
        BookmarkCommand::List => {
            let bookmarks = list_bookmarks(&ctx.db)?;
            if out.mode() == OutputMode::Json {
                out.json(&json!({
                    "bookmarks": bookmarks.iter().map(|b| {
                        json!({
                            "id": b.id,
                            "surah_no": b.surah_no,
                            "ayah_no": b.ayah_no,
                            "note": b.note,
                            "created_at": b.created_at,
                        })
                    }).collect::<Vec<_>>()
                }))?;
            } else if bookmarks.is_empty() {
                out.status("INFO", "Belum ada bookmark.");
            } else {
                out.title("Bookmark List");
                let rows: Vec<Vec<String>> = bookmarks
                    .into_iter()
                    .map(|b| {
                        vec![
                            b.id.to_string(),
                            b.surah_no.to_string(),
                            b.ayah_no.to_string(),
                            b.note.unwrap_or_else(|| "-".to_string()),
                            b.created_at,
                        ]
                    })
                    .collect();
                out.table(&["ID", "Surah", "Ayah", "Note", "Created"], &rows);
            }
        }
        BookmarkCommand::Remove { bookmark_id } => {
            if !ctx.assume_yes {
                if !std::io::stdin().is_terminal() {
                    return Err(anyhow!(
                        "Menghapus bookmark butuh konfirmasi.\nHint: pakai `--yes` untuk mode non-interaktif."
                    ));
                }
                let confirmed = Confirm::new(&format!(
                    "Hapus bookmark #{bookmark_id}? Aksi ini tidak bisa dibatalkan."
                ))
                .with_default(false)
                .prompt()?;
                if !confirmed {
                    out.status("INFO", "Penghapusan dibatalkan.");
                    return Ok(());
                }
            }

            let typed_id = BookmarkId::new(bookmark_id).context("ID bookmark harus > 0")?;
            let deleted = remove_bookmark(&ctx.db, typed_id)?;
            if out.mode() == OutputMode::Json {
                out.json(&json!({"bookmark_id": bookmark_id, "deleted": deleted}))?;
            } else if deleted == 0 {
                out.status("WARN", format!("Bookmark #{bookmark_id} tidak ditemukan."));
            } else {
                out.status("OK", format!("Bookmark #{bookmark_id} dihapus."));
            }
        }
    }
    Ok(())
}

fn run_continue(ctx: &mut AppContext) -> Result<()> {
    let lang = LanguageTag::new(&ctx.config.default_lang)
        .context("default_lang config tidak valid untuk command continue")?;
    let output = continue_reading(&ctx.api, &ctx.db, ContinueInput { lang })
        .context("Gagal melanjutkan progress. Pastikan progress sudah tersimpan.")?;

    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    if out.mode() == OutputMode::Json {
        out.json(&json!({"ayah": ayah_to_json(&output.ayah)}))?;
        return Ok(());
    }

    out.title("Continue");
    out.kv(
        "Posisi",
        format!("{}:{}", output.ayah.surah_no, output.ayah.ayah_no),
    );
    out.line(output.ayah.arabic_text);
    out.line(output.ayah.translation.unwrap_or_else(|| "-".to_string()));
    Ok(())
}

fn run_plan(ctx: &mut AppContext, count: usize) -> Result<()> {
    let lang =
        LanguageTag::new(&ctx.config.default_lang).context("default_lang config tidak valid")?;
    let output = daily_plan(
        &ctx.api,
        &ctx.db,
        PlanInput {
            count: PlanCount::new(count)?,
            lang,
        },
    )
    .context("Gagal menyusun rencana harian. Cek koneksi internet lalu coba lagi.")?;

    let out = Output::new(ctx.output_mode, ctx.color_enabled);
    if out.mode() == OutputMode::Json {
        out.json(&json!({
            "count": count,
            "ayahs": output.ayahs.iter().map(ayah_to_json).collect::<Vec<_>>()
        }))?;
        return Ok(());
    }

    if output.ayahs.is_empty() {
        out.status("INFO", "Belum ada rencana hari ini.");
        return Ok(());
    }

    out.title(&format!("Rencana hari ini ({count} ayat)"));
    let rows: Vec<Vec<String>> = output
        .ayahs
        .into_iter()
        .map(|ayah| {
            let text = ayah.translation.unwrap_or_else(|| ayah.arabic_text.clone());
            vec![ayah.surah_no.to_string(), ayah.ayah_no.to_string(), text]
        })
        .collect();
    out.table(&["Surah", "Ayah", "Text"], &rows);
    Ok(())
}

fn run_sync(ctx: &mut AppContext, force: bool) -> Result<()> {
    let sync_input = SyncInput::new(force, ctx.config.sync.enabled);
    if !sync_input.should_run() {
        return render_sync_output(
            ctx.output_mode,
            ctx.color_enabled,
            SyncOutput {
                status: SyncStatus::Skipped,
            },
        );
    }

    let runnable = sync_input
        .into_runnable()
        .ok_or_else(|| anyhow!("gagal mempromosikan sync input ke state runnable"))?;

    let output = if ctx.output_mode == OutputMode::Json {
        sync_content(&ctx.db, &ctx.api, runnable)?
    } else {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::with_template("{spinner} {msg}")?);
        spinner.enable_steady_tick(Duration::from_millis(100));
        spinner.set_message("Mengecek metadata konten online...");

        let output = sync_content(&ctx.db, &ctx.api, runnable)?;
        match output.status {
            SyncStatus::Skipped => spinner.finish_and_clear(),
            SyncStatus::Success(_) => spinner.finish_with_message("Sync metadata berhasil"),
            SyncStatus::Failure(_) => spinner.finish_with_message("Sync gagal"),
        }
        output
    };

    render_sync_output(ctx.output_mode, ctx.color_enabled, output)
}

fn render_sync_output(mode: OutputMode, color_enabled: bool, output: SyncOutput) -> Result<()> {
    let out = Output::new(mode, color_enabled);
    match output.status {
        SyncStatus::Skipped => {
            if out.mode() == OutputMode::Json {
                out.json(&json!({"status": "skipped", "message": "Sync dinonaktifkan."}))?;
            } else {
                out.status(
                    "INFO",
                    "Sync dinonaktifkan di config. Gunakan --force untuk bypass.",
                );
            }
        }
        SyncStatus::Success(message) => {
            if out.mode() == OutputMode::Json {
                out.json(&json!({"status": "success", "message": message}))?;
            } else {
                out.status("OK", message);
            }
        }
        SyncStatus::Failure(message) => {
            if out.mode() == OutputMode::Json {
                out.json(&json!({"status": "failure", "message": message}))?;
            } else {
                out.status("ERR", message);
            }
        }
    }
    Ok(())
}

fn run_config(ctx: &mut AppContext, cmd: ConfigCommand) -> Result<()> {
    let out = Output::new(ctx.output_mode, ctx.color_enabled);

    match cmd {
        ConfigCommand::Set { key, value } => {
            ctx.config.set_key(&key, &value)?;
            save(&ctx.paths, &ctx.config)?;
            if out.mode() == OutputMode::Json {
                out.json(&json!({"updated": key, "value": value}))?;
            } else {
                out.status("OK", format!("Config diperbarui: {key}={value}"));
            }
        }
        ConfigCommand::Show => {
            if out.mode() == OutputMode::Json {
                let raw = toml::to_string(&ctx.config)?;
                let parsed: Value = toml::from_str(&raw)?;
                out.json(&parsed)?;
            } else {
                let raw = toml::to_string_pretty(&ctx.config)?;
                out.line(raw);
            }
        }
    }
    Ok(())
}

fn with_spinner<T, F>(mode: OutputMode, message: &str, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    if mode == OutputMode::Json || mode == OutputMode::Plain {
        return f();
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::with_template("{spinner} {msg}")?);
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message(message.to_string());
    let result = f();
    match &result {
        Ok(_) => spinner.finish_and_clear(),
        Err(_) => spinner.finish_with_message("Operasi gagal."),
    }
    result
}

fn build_read_input(surah: u16, ayah: Option<u16>, lang: &str) -> Result<ReadInput> {
    let target = build_read_target(surah, ayah)?;
    let parsed_lang = LanguageTag::new(lang)?;
    Ok(match target {
        ReadTarget::Surah(value) => ReadInput::for_surah(value, parsed_lang),
        ReadTarget::Ayah(value) => ReadInput::for_ayah(value, parsed_lang),
    })
}

fn build_read_target(surah: u16, ayah: Option<u16>) -> Result<ReadTarget> {
    let surah = SurahNumber::new(surah)?;
    Ok(match ayah {
        Some(value) => ReadTarget::Ayah(AyahRef::new(surah, AyahNumber::new(value)?)),
        None => ReadTarget::Surah(surah),
    })
}

fn ayah_to_json(ayah: &crate::domain::Ayah) -> Value {
    json!({
        "surah_no": ayah.surah_no,
        "ayah_no": ayah.ayah_no,
        "arabic_text": ayah.arabic_text,
        "transliteration": ayah.transliteration,
        "translation": ayah.translation,
        "audio_url": ayah.audio_url,
        "audio_urls": ayah.audio_urls,
    })
}
