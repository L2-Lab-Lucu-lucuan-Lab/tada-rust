use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, anyhow};
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::BorderType;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::application::ports::{BookmarkRepository, ProgressRepository, QuranReadRepository};
use crate::audio::{AudioCache, AudioPlayer, PlayerTick, qari_name};
use crate::domain::{
    Ayah, AyahNumber, AyahRef, Bookmark, BookmarkId, LanguageTag, SearchHit, SearchLimit,
    SurahMeta, SurahNumber,
};

mod actions;
mod input;
mod render;
mod theme;

use actions::{apply_intent, ayah_ref_from_raw, sync_audio_tick};
use input::map_key_to_intent;
use render::{can_show_sidebar_in_frame, draw_ui, filter_surah_indices, frame_size_hint};
use theme::{Theme, default_themes, load_themes};

#[derive(Debug, Clone)]
pub struct TuiLaunchOptions {
    pub theme_mode: String,
    pub show_translation: bool,
    pub audio_cache_root: PathBuf,
    pub default_qari: String,
    pub cache_enabled: bool,
    pub cache_max_mb: u64,
    pub initial_surah: Option<u16>,
    pub initial_ayah: Option<u16>,
    pub autoplay: bool,
    pub qari_override: Option<String>,
    pub color_enabled: bool,
}

pub fn run_tui<R, S>(
    read_repo: &R,
    state_repo: &S,
    lang: &str,
    options: &TuiLaunchOptions,
) -> Result<()>
where
    R: QuranReadRepository,
    S: ProgressRepository + BookmarkRepository,
{
    let mut terminal = init_terminal()?;
    let lang = LanguageTag::new(lang)?;

    let result = (|| -> Result<()> {
        let surahs = read_repo.list_surahs()?;
        if surahs.is_empty() {
            return Err(anyhow!("Tidak ada data surah dari API"));
        }

        let audio_cache = AudioCache::new(
            options.audio_cache_root.clone(),
            options.cache_enabled,
            options.cache_max_mb,
        )?;
        let mut state = TuiState::new(
            surahs,
            &options.theme_mode,
            options.color_enabled,
            options.show_translation,
            audio_cache,
            options
                .qari_override
                .clone()
                .unwrap_or_else(|| options.default_qari.clone()),
        );
        if let Some(surah) = options.initial_surah
            && let Some(idx) = state.surahs.iter().position(|s| s.surah_no == surah)
        {
            state.selected_surah_idx = idx;
            state.surah_cursor_idx = idx;
        }
        state.load_surah(read_repo, &lang)?;
        if let Some(ayah) = options.initial_ayah {
            state.selected_ayah_idx = ayah.saturating_sub(1) as usize;
            state.selected_ayah_idx = state
                .selected_ayah_idx
                .min(state.ayahs.len().saturating_sub(1));
        }
        if options.autoplay {
            state.start_or_toggle_playback()?;
        }

        loop {
            terminal.autoresize()?;
            sync_audio_tick(state_repo, &mut state)?;
            terminal.draw(|frame| draw_ui(frame, &state))?;

            if event::poll(Duration::from_millis(120))? {
                match event::read()? {
                    Event::Resize(_, _) => {
                        terminal.autoresize()?;
                    }
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }

                        let (terminal_width, terminal_height) = frame_size_hint(&terminal);
                        if let Some(intent) = map_key_to_intent(&state, key)
                            && apply_intent(
                                read_repo,
                                state_repo,
                                &lang,
                                &mut state,
                                intent,
                                terminal_width,
                                terminal_height,
                            )?
                        {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(current) = state.current_ayah() {
            state_repo.set_progress(ayah_ref_from_raw(current.surah_no, current.ayah_no)?)?;
        }

        Ok(())
    })();

    restore_terminal(&mut terminal)?;
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Reading,
    CommandPalette,
    SearchInline,
    BookmarkOverlay,
    HelpOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    SurahCards,
    AyahReader,
}

#[derive(Debug, Clone, Copy)]
enum Intent {
    Quit,
    NextAyah,
    PrevAyah,
    NextSurah,
    PrevSurah,
    TogglePlayback,
    AudioNext,
    AudioPrev,
    AudioStop,
    AudioRepeat,
    AudioSpeedDown,
    AudioSpeedUp,
    CycleQari,
    FocusNextPane,
    SurahCursorUp,
    SurahCursorDown,
    SurahCursorSelect,
    SurahFilterType(char),
    SurahFilterBackspace,
    SurahFilterClear,
    ToggleSidebar,
    ToggleHelp,
    OpenPalette,
    OpenSearch,
    OpenBookmarks,
    AddBookmark,
    RemoveCurrentAyahBookmarks,
    CloseOverlay,
    PaletteMoveUp,
    PaletteMoveDown,
    PaletteBackspace,
    PaletteSubmit,
    PaletteType(char),
    SearchMoveUp,
    SearchMoveDown,
    SearchBackspace,
    SearchSubmit,
    SearchType(char),
    BookmarkDelete,
    BookmarkMoveUp,
    BookmarkMoveDown,
    BookmarkJump,
    CycleTheme,
}

#[derive(Debug, Clone, Copy)]
enum PaletteAction {
    ToggleSidebar,
    OpenBookmarks,
    OpenSearch,
    ToggleHelp,
    TogglePlayback,
    StopPlayback,
    AddBookmark,
    NextSurah,
    PrevSurah,
    Quit,
    CycleTheme,
}

#[derive(Debug, Clone, Copy)]
struct PaletteCommand {
    label: &'static str,
    keywords: &'static str,
    action: PaletteAction,
}

const PALETTE_COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        label: "Toggle side panel",
        keywords: "panel sidebar",
        action: PaletteAction::ToggleSidebar,
    },
    PaletteCommand {
        label: "Open bookmarks",
        keywords: "bookmark list",
        action: PaletteAction::OpenBookmarks,
    },
    PaletteCommand {
        label: "Search ayah",
        keywords: "search find query",
        action: PaletteAction::OpenSearch,
    },
    PaletteCommand {
        label: "Toggle help",
        keywords: "help shortcut",
        action: PaletteAction::ToggleHelp,
    },
    PaletteCommand {
        label: "Play or pause audio",
        keywords: "play pause running audio",
        action: PaletteAction::TogglePlayback,
    },
    PaletteCommand {
        label: "Stop playback",
        keywords: "stop audio",
        action: PaletteAction::StopPlayback,
    },
    PaletteCommand {
        label: "Add bookmark",
        keywords: "bookmark save",
        action: PaletteAction::AddBookmark,
    },
    PaletteCommand {
        label: "Next surah",
        keywords: "surah next",
        action: PaletteAction::NextSurah,
    },
    PaletteCommand {
        label: "Previous surah",
        keywords: "surah prev",
        action: PaletteAction::PrevSurah,
    },
    PaletteCommand {
        label: "Quit",
        keywords: "exit close",
        action: PaletteAction::Quit,
    },
    PaletteCommand {
        label: "Cycle theme",
        keywords: "theme color",
        action: PaletteAction::CycleTheme,
    },
];

struct ThemeStyles {
    app_bg: Style,
    frame: Style,
    muted: Style,
    accent: Style,
    strong: Style,
    panel: Style,
    card: Style,
    card_active: Style,
    card_focus: Style,
    chip: Style,
}

impl ThemeStyles {
    fn from_theme(theme: &Theme, color_enabled: bool) -> Self {
        if !color_enabled {
            return Self {
                app_bg: Style::default(),
                frame: Style::default(),
                muted: Style::default(),
                accent: Style::default().add_modifier(Modifier::BOLD),
                strong: Style::default().add_modifier(Modifier::BOLD),
                panel: Style::default(),
                card: Style::default(),
                card_active: Style::default().add_modifier(Modifier::BOLD),
                card_focus: Style::default().add_modifier(Modifier::BOLD),
                chip: Style::default().add_modifier(Modifier::BOLD),
            };
        }

        Self {
            app_bg: Style::default().bg(theme.bg),
            frame: Style::default().fg(theme.fg),
            muted: Style::default().fg(Color::Gray),
            accent: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            strong: Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            panel: Style::default().fg(theme.fg).bg(theme.bg),
            card: Style::default().fg(theme.fg).bg(theme.bg),
            card_active: Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
            card_focus: Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
            chip: Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        }
    }
}

struct TuiState {
    surahs: Vec<SurahMeta>,
    selected_surah_idx: usize,
    ayahs: Vec<Ayah>,
    selected_ayah_idx: usize,
    bookmarks: Vec<Bookmark>,
    selected_bookmark_idx: usize,
    search_results: Vec<SearchHit>,
    selected_search_idx: usize,
    status: String,
    mode: UiMode,
    sidebar_collapsed: bool,
    focus: PaneFocus,
    surah_cursor_idx: usize,
    surah_filter: String,
    show_translation: bool,
    palette_input: String,
    palette_selected_idx: usize,
    search_input: String,
    themes: Vec<Theme>,
    theme_idx: usize,
    theme: ThemeStyles,
    audio_cache: AudioCache,
    active_qari: String,
    color_enabled: bool,
    player: Option<AudioPlayer>,
}

impl TuiState {
    fn new(
        surahs: Vec<SurahMeta>,
        theme_mode: &str,
        color_enabled: bool,
        show_translation: bool,
        audio_cache: AudioCache,
        active_qari: String,
    ) -> Self {
        let themes = load_themes("theme.yml").unwrap_or_else(|_| default_themes());
        let theme_idx = themes
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(theme_mode))
            .unwrap_or(0);
        let current_theme = &themes[theme_idx];

        Self {
            surahs,
            selected_surah_idx: 0,
            ayahs: Vec::new(),
            selected_ayah_idx: 0,
            bookmarks: Vec::new(),
            selected_bookmark_idx: 0,
            search_results: Vec::new(),
            selected_search_idx: 0,
            status: "j/k navigasi ayat | Ctrl+B panel surat | Space play/pause | / search"
                .to_string(),
            mode: UiMode::Reading,
            sidebar_collapsed: true,
            focus: PaneFocus::AyahReader,
            surah_cursor_idx: 0,
            surah_filter: String::new(),
            show_translation,
            palette_input: String::new(),
            palette_selected_idx: 0,
            search_input: String::new(),
            themes: themes.clone(),
            theme_idx,
            theme: ThemeStyles::from_theme(current_theme, color_enabled),
            audio_cache,
            active_qari,
            color_enabled,
            player: None,
        }
    }

    fn current_theme(&self) -> &Theme {
        &self.themes[self.theme_idx]
    }

    fn set_theme(&mut self, idx: usize) {
        if idx < self.themes.len() {
            self.theme_idx = idx;
            self.theme = ThemeStyles::from_theme(&self.themes[self.theme_idx], self.color_enabled);
        }
    }

    fn next_theme(&mut self) {
        let next = (self.theme_idx + 1) % self.themes.len();
        self.set_theme(next);
    }

    fn current_surah(&self) -> &SurahMeta {
        &self.surahs[self.selected_surah_idx]
    }

    fn current_ayah(&self) -> Option<&Ayah> {
        self.ayahs.get(self.selected_ayah_idx)
    }

    fn load_surah<R>(&mut self, repo: &R, lang: &LanguageTag) -> Result<()>
    where
        R: QuranReadRepository,
    {
        let surah_no = SurahNumber::new(self.current_surah().surah_no)?;
        self.ayahs = repo.read_surah(surah_no, lang)?;
        self.selected_ayah_idx = self
            .selected_ayah_idx
            .min(self.ayahs.len().saturating_sub(1));
        self.status = format!(
            "Membaca {} [{} ayat]",
            self.current_surah().name_id,
            self.current_surah().ayah_count
        );
        Ok(())
    }

    fn filtered_palette(&self) -> Vec<PaletteCommand> {
        if self.palette_input.trim().is_empty() {
            return PALETTE_COMMANDS.to_vec();
        }
        let needle = self.palette_input.to_lowercase();
        PALETTE_COMMANDS
            .iter()
            .copied()
            .filter(|item| {
                item.label.to_lowercase().contains(&needle)
                    || item.keywords.to_lowercase().contains(&needle)
            })
            .collect()
    }

    fn jump_to(&mut self, surah_no: u16, ayah_no: u16) {
        if let Some(idx) = self.surahs.iter().position(|s| s.surah_no == surah_no) {
            self.selected_surah_idx = idx;
            self.surah_cursor_idx = idx;
            self.selected_ayah_idx = ayah_no.saturating_sub(1) as usize;
        }
    }

    fn filtered_surah_indices(&self) -> Vec<usize> {
        filter_surah_indices(&self.surahs, &self.surah_filter)
    }

    fn clamp_surah_cursor_to_filter(&mut self) {
        let filtered = self.filtered_surah_indices();
        if filtered.is_empty() {
            return;
        }

        if !filtered.contains(&self.surah_cursor_idx) {
            self.surah_cursor_idx = filtered[0];
        }
    }

    fn start_or_toggle_playback(&mut self) -> Result<()> {
        if let Some(player) = &mut self.player {
            player.toggle_pause();
            self.status = if player.is_paused() {
                "Audio pause. j/k pindah ayat, Space lanjut.".to_string()
            } else {
                "Audio lanjut".to_string()
            };
            return Ok(());
        }

        if self.ayahs.is_empty() || self.selected_ayah_idx >= self.ayahs.len() {
            self.status = "Tidak ada ayat untuk diputar".to_string();
            return Ok(());
        }

        let playlist = self.ayahs[self.selected_ayah_idx..].to_vec();
        let player = AudioPlayer::new(
            playlist,
            0,
            self.audio_cache.clone(),
            Some(self.active_qari.clone()),
            self.active_qari.clone(),
        )?;
        if let Some(ayah) = player.current_ayah() {
            self.status = format!(
                "PLAY {}:{} | qari {}",
                ayah.surah_no, ayah.ayah_no, self.active_qari
            );
            self.selected_ayah_idx = ayah.ayah_no.saturating_sub(1) as usize;
        }
        self.player = Some(player);
        Ok(())
    }

    fn stop_playback(&mut self) {
        if let Some(player) = &mut self.player {
            player.stop();
        }
        self.player = None;
        self.status = "Audio berhenti".to_string();
    }
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fullscreen,
        },
    )?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
