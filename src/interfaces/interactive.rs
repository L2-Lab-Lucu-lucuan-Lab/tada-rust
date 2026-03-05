use anyhow::Result;
use inquire::validator::{ErrorMessage, Validation};
use inquire::{Confirm, CustomType, Select, Text};

use crate::cli::{
    BookmarkAddArgs, BookmarkCommand, Command, ConfigCommand, PlayArgs, ReadArgs, SearchArgs,
    SearchScope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardDecision {
    Execute,
    Exit,
}

pub fn prompt_command(
    default_lang: &str,
    assume_yes: bool,
) -> Result<(WizardDecision, Option<Command>)> {
    let actions = vec![
        "Baca ayat/surah",
        "Putar audio",
        "Cari ayat",
        "Tambah bookmark",
        "Lihat bookmark",
        "Hapus bookmark",
        "Lanjutkan progress",
        "Rencana harian",
        "Sync metadata",
        "Lihat konfigurasi",
        "Ubah konfigurasi",
        "Buka TUI layar penuh",
        "Doctor",
        "Keluar",
    ];

    let choice = Select::new("Pilih aksi", actions)
        .with_page_size(14)
        .prompt()?;

    let command = match choice {
        "Baca ayat/surah" => Some(prompt_read(default_lang)?),
        "Putar audio" => Some(prompt_play()?),
        "Cari ayat" => Some(prompt_search()?),
        "Tambah bookmark" => Some(prompt_bookmark_add()?),
        "Lihat bookmark" => Some(Command::Bookmark {
            command: BookmarkCommand::List,
        }),
        "Hapus bookmark" => prompt_bookmark_remove(assume_yes)?,
        "Lanjutkan progress" => Some(Command::Continue),
        "Rencana harian" => Some(prompt_plan()?),
        "Sync metadata" => Some(prompt_sync()?),
        "Lihat konfigurasi" => Some(Command::Config {
            command: ConfigCommand::Show,
        }),
        "Ubah konfigurasi" => Some(prompt_config_set()?),
        "Buka TUI layar penuh" => Some(Command::Tui),
        "Doctor" => Some(Command::Doctor),
        "Keluar" => {
            return Ok((WizardDecision::Exit, None));
        }
        _ => {
            return Ok((WizardDecision::Exit, None));
        }
    };

    Ok((WizardDecision::Execute, command))
}

fn prompt_read(default_lang: &str) -> Result<Command> {
    let surah = prompt_u16("Nomor surah (1..114)", 1, 114, None)?;
    let with_ayah = Confirm::new("Baca ayat spesifik?")
        .with_default(false)
        .prompt()?;
    let ayah = if with_ayah {
        Some(prompt_u16("Nomor ayat (1..286)", 1, 286, None)?)
    } else {
        None
    };
    let lang = Text::new("Bahasa terjemahan")
        .with_default(default_lang)
        .prompt()?;
    Ok(Command::Read(ReadArgs { surah, ayah, lang }))
}

fn prompt_play() -> Result<Command> {
    let surah = prompt_u16("Nomor surah (1..114)", 1, 114, None)?;
    let with_ayah = Confirm::new("Putar ayat spesifik?")
        .with_default(false)
        .prompt()?;
    let ayah = if with_ayah {
        Some(prompt_u16("Nomor ayat (1..286)", 1, 286, None)?)
    } else {
        None
    };

    let use_qari_override = Confirm::new("Pakai override qari?")
        .with_default(false)
        .prompt()?;
    let qari = if use_qari_override {
        Some(
            Text::new("Qari (01..06 atau nama singkat)")
                .with_default("05")
                .prompt()?,
        )
    } else {
        None
    };

    let open = Confirm::new("Buka audio di browser/player default?")
        .with_default(false)
        .prompt()?;

    let no_cache = Confirm::new("Bypass cache audio untuk perintah ini?")
        .with_default(false)
        .prompt()?;

    Ok(Command::Play(PlayArgs {
        surah,
        ayah,
        qari,
        no_cache,
        open,
    }))
}

fn prompt_search() -> Result<Command> {
    let query = Text::new("Masukkan kata kunci pencarian").prompt()?;
    let scope_raw =
        Select::new("Pilih scope", vec!["Semua", "Teks Quran", "Terjemahan"]).prompt()?;
    let scope = match scope_raw {
        "Semua" => SearchScope::All,
        "Teks Quran" => SearchScope::Quran,
        "Terjemahan" => SearchScope::Translation,
        _ => SearchScope::All,
    };
    let limit = prompt_u16("Jumlah hasil maksimum (1..200)", 1, 200, Some(20))?;
    Ok(Command::Search(SearchArgs {
        query,
        scope,
        limit,
    }))
}

fn prompt_bookmark_add() -> Result<Command> {
    let surah = prompt_u16("Nomor surah (1..114)", 1, 114, None)?;
    let ayah = prompt_u16("Nomor ayat (1..286)", 1, 286, None)?;
    let note_raw = Text::new("Catatan (boleh kosong)")
        .with_default("")
        .prompt()?;
    let note = if note_raw.trim().is_empty() {
        None
    } else {
        Some(note_raw)
    };
    Ok(Command::Bookmark {
        command: BookmarkCommand::Add(BookmarkAddArgs { surah, ayah, note }),
    })
}

fn prompt_bookmark_remove(assume_yes: bool) -> Result<Option<Command>> {
    let bookmark_id = CustomType::<i64>::new("ID bookmark")
        .with_error_message("Masukkan angka bulat positif.")
        .with_validator(|v: &i64| {
            if *v > 0 {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "ID bookmark harus > 0".to_string(),
                )))
            }
        })
        .prompt()?;

    let confirmed = if assume_yes {
        true
    } else {
        Confirm::new("Yakin ingin menghapus bookmark ini?")
            .with_default(false)
            .prompt()?
    };

    if !confirmed {
        return Ok(None);
    }

    Ok(Some(Command::Bookmark {
        command: BookmarkCommand::Remove { bookmark_id },
    }))
}

fn prompt_plan() -> Result<Command> {
    let count = CustomType::<usize>::new("Jumlah ayat rencana (1..1000)")
        .with_default(5)
        .with_error_message("Masukkan angka 1..1000")
        .with_validator(|v: &usize| {
            if (1..=1000).contains(v) {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Nilai harus 1..1000".to_string(),
                )))
            }
        })
        .prompt()?;
    Ok(Command::Plan { count })
}

fn prompt_sync() -> Result<Command> {
    let force = Confirm::new("Paksa sync walau sync.enabled=false?")
        .with_default(false)
        .prompt()?;
    Ok(Command::Sync { force })
}

fn prompt_config_set() -> Result<Command> {
    let key = Text::new("Key config (contoh: ui.output)")
        .with_help_message("Lihat `tada-rust config show` untuk key aktif.")
        .prompt()?;
    let value = Text::new("Value").prompt()?;
    Ok(Command::Config {
        command: ConfigCommand::Set { key, value },
    })
}

fn prompt_u16(label: &str, min: u16, max: u16, default: Option<u16>) -> Result<u16> {
    let mut prompt = CustomType::<u16>::new(label)
        .with_error_message("Masukkan angka yang valid.")
        .with_validator(move |v: &u16| {
            if (min..=max).contains(v) {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(format!(
                    "Nilai harus {min}..{max}"
                ))))
            }
        });

    if let Some(value) = default {
        prompt = prompt.with_default(value);
    }

    prompt.prompt().map_err(Into::into)
}
