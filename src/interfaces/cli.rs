use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "tada-rust",
    about = "Asisten interaktif belajar Al-Qur'an dari terminal",
    long_about = "Asisten belajar Al-Qur'an dengan mode CLI, wizard interaktif, dan TUI satu layar.",
    after_long_help = ROOT_EXAMPLES,
    version
)]
pub struct Cli {
    #[arg(long, global = true, help = "Override direktori data aplikasi")]
    pub data_dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        default_value_t = false,
        help = "Output minimal tanpa styling rich"
    )]
    pub plain: bool,
    #[arg(
        long,
        global = true,
        default_value_t = false,
        help = "Output JSON untuk integrasi scripting"
    )]
    pub json: bool,
    #[arg(
        short = 'v',
        long,
        global = true,
        action = ArgAction::Count,
        help = "Naikkan level log (pakai -vv untuk lebih detail)"
    )]
    pub verbose: u8,
    #[arg(
        long,
        global = true,
        default_value_t = false,
        help = "Aktifkan log debug (setara RUST_LOG=debug)"
    )]
    pub debug: bool,
    #[arg(
        long,
        global = true,
        default_value_t = false,
        help = "Matikan warna pada output CLI"
    )]
    pub no_color: bool,
    #[arg(
        short = 'y',
        long,
        global = true,
        default_value_t = false,
        help = "Jawab ya otomatis untuk aksi destruktif"
    )]
    pub yes: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(
        about = "Wizard menu interaktif (prompt berbasis langkah)",
        alias = "wizard",
        after_long_help = "Contoh:\n  tada-rust interactive\n  tada-rust interactive --plain"
    )]
    Interactive,
    #[command(
        about = "Baca satu ayat atau satu surah",
        after_long_help = READ_EXAMPLES
    )]
    Read(ReadArgs),
    #[command(
        about = "Putar audio ayat/surah, atau buka URL audio",
        after_long_help = PLAY_EXAMPLES
    )]
    Play(PlayArgs),
    #[command(
        about = "Cari teks ayat/terjemahan",
        after_long_help = SEARCH_EXAMPLES
    )]
    Search(SearchArgs),
    #[command(
        about = "Kelola bookmark ayat",
        after_long_help = BOOKMARK_EXAMPLES
    )]
    Bookmark {
        #[command(subcommand)]
        command: BookmarkCommand,
    },
    #[command(
        about = "Lanjutkan dari progress terakhir",
        after_long_help = "Contoh:\n  tada-rust continue\n  tada-rust continue --json"
    )]
    Continue,
    #[command(
        about = "Susun rencana bacaan harian",
        after_long_help = PLAN_EXAMPLES
    )]
    Plan {
        #[arg(long, default_value_t = 5, value_parser = parse_plan_count)]
        count: usize,
    },
    #[command(
        about = "Sinkronisasi metadata dan cek koneksi API",
        after_long_help = "Contoh:\n  tada-rust sync\n  tada-rust sync --force"
    )]
    Sync {
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    #[command(
        about = "Jalankan terminal UI layar penuh",
        after_long_help = "Contoh:\n  tada-rust tui\n  tada-rust tui --data-dir ./tmp/tada"
    )]
    Tui,
    #[command(
        about = "Lihat/ubah konfigurasi aplikasi",
        after_long_help = CONFIG_EXAMPLES
    )]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(
        about = "Diagnostik lingkungan lokal",
        after_long_help = "Contoh:\n  tada-rust doctor\n  tada-rust doctor --json"
    )]
    Doctor,
}

#[derive(Debug, Args)]
pub struct ReadArgs {
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=114))]
    pub surah: u16,
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=286))]
    pub ayah: Option<u16>,
    #[arg(long, default_value = "id")]
    pub lang: String,
}

#[derive(Debug, Args)]
pub struct PlayArgs {
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=114))]
    pub surah: u16,
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=286))]
    pub ayah: Option<u16>,
    #[arg(
        long,
        value_name = "QARI_ID",
        help = "Pilih qari 01..06 (override default config)"
    )]
    pub qari: Option<String>,
    #[arg(
        long,
        default_value_t = false,
        help = "Bypass cache audio untuk command ini"
    )]
    pub no_cache: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Buka URL audio di player/browser default"
    )]
    pub open: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SearchScope {
    Quran,
    Translation,
    All,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long, value_enum, default_value_t = SearchScope::All)]
    pub scope: SearchScope,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=200))]
    pub limit: u16,
}

#[derive(Debug, Subcommand)]
pub enum BookmarkCommand {
    #[command(
        about = "Tambahkan bookmark baru",
        after_long_help = "Contoh:\n  tada-rust bookmark add --surah 2 --ayah 255 --note \"Ayat kursi\""
    )]
    Add(BookmarkAddArgs),
    #[command(
        about = "Daftar semua bookmark",
        after_long_help = "Contoh:\n  tada-rust bookmark list\n  tada-rust bookmark list --json"
    )]
    List,
    #[command(
        about = "Hapus bookmark berdasarkan ID",
        after_long_help = "Contoh:\n  tada-rust bookmark remove 4\n  tada-rust bookmark remove 4 --yes"
    )]
    Remove { bookmark_id: i64 },
}

#[derive(Debug, Args)]
pub struct BookmarkAddArgs {
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=114))]
    pub surah: u16,
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=286))]
    pub ayah: u16,
    #[arg(long)]
    pub note: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    #[command(
        about = "Set nilai konfigurasi",
        after_long_help = "Contoh:\n  tada-rust config set default_lang id\n  tada-rust config set audio.default_qari 05"
    )]
    Set { key: String, value: String },
    #[command(
        about = "Tampilkan konfigurasi aktif",
        after_long_help = "Contoh:\n  tada-rust config show\n  tada-rust config show --json"
    )]
    Show,
}

const ROOT_EXAMPLES: &str = "Contoh umum:
  tada-rust interactive
  tada-rust read --surah 1 --ayah 1
  tada-rust search rahmat --scope translation
  tada-rust play --surah 18 --ayah 10 --qari 05
  tada-rust bookmark list
  tada-rust tui
  tada-rust doctor --json";

const READ_EXAMPLES: &str = "Contoh:
  tada-rust read --surah 1 --ayah 1
  tada-rust read --surah 55 --lang id
  tada-rust read --surah 2 --ayah 255 --json";

const PLAY_EXAMPLES: &str = "Contoh:
  tada-rust play --surah 1 --ayah 1
  tada-rust play --surah 67 --qari 03
  tada-rust play --surah 18 --open";

const SEARCH_EXAMPLES: &str = "Contoh:
  tada-rust search sabar
  tada-rust search rahmat --scope translation --limit 30
  tada-rust search alhamdu --scope quran";

const BOOKMARK_EXAMPLES: &str = "Contoh:
  tada-rust bookmark add --surah 2 --ayah 255 --note \"Ayat kursi\"
  tada-rust bookmark list
  tada-rust bookmark remove 12 --yes";

const PLAN_EXAMPLES: &str = "Contoh:
  tada-rust plan
  tada-rust plan --count 10
  tada-rust plan --count 20 --json";

const CONFIG_EXAMPLES: &str = "Contoh:
  tada-rust config show
  tada-rust config set ui.output rich
  tada-rust config set audio.cache_max_mb 1024";

fn parse_plan_count(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "count harus berupa angka".to_string())?;
    if (1..=1000).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("count harus 1..1000".to_string())
    }
}
