use std::path::PathBuf;
use std::{fmt::Debug, marker::PhantomData};

use crate::domain::{
    Ayah, AyahRef, LanguageTag, PlanCount, QariId, SearchHit, SearchLimit, SurahNumber,
};

#[derive(Debug, Clone)]
pub enum SearchScope {
    Quran,
    Translation,
    All,
}

#[derive(Debug, Clone)]
pub struct ReadInput {
    pub target: ReadTarget,
    pub lang: LanguageTag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadTarget {
    Surah(SurahNumber),
    Ayah(AyahRef),
}

impl ReadInput {
    #[must_use]
    pub fn for_surah(surah: SurahNumber, lang: LanguageTag) -> Self {
        Self {
            target: ReadTarget::Surah(surah),
            lang,
        }
    }

    #[must_use]
    pub fn for_ayah(target: AyahRef, lang: LanguageTag) -> Self {
        Self {
            target: ReadTarget::Ayah(target),
            lang,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayInput {
    pub target: ReadTarget,
    pub lang: LanguageTag,
    pub qari: Option<QariId>,
    pub fallback_qari: QariId,
}

#[derive(Debug, Clone)]
pub struct PlayOutput {
    pub ayahs: Vec<Ayah>,
    pub target_audio: Option<String>,
    pub selected_qari: Option<QariId>,
}

#[derive(Debug, Clone)]
pub enum ReadOutput {
    Single(Ayah),
    Surah(Vec<Ayah>),
}

#[derive(Debug, Clone)]
pub struct SearchInput {
    pub query: String,
    pub scope: SearchScope,
    pub limit: SearchLimit,
}

#[derive(Debug, Clone)]
pub struct SearchOutput {
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone)]
pub struct BookmarkAddInput {
    pub target: AyahRef,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContinueInput {
    pub lang: LanguageTag,
}

#[derive(Debug, Clone)]
pub struct ContinueOutput {
    pub ayah: Ayah,
}

#[derive(Debug, Clone)]
pub struct PlanInput {
    pub count: PlanCount,
    pub lang: LanguageTag,
}

#[derive(Debug, Clone)]
pub struct PlanOutput {
    pub ayahs: Vec<Ayah>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncUnchecked;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncRunnable;

#[derive(Clone)]
pub struct SyncInput<State = SyncUnchecked> {
    force: bool,
    sync_enabled: bool,
    _state: PhantomData<State>,
}

impl<State> Debug for SyncInput<State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncInput")
            .field("force", &self.force)
            .field("sync_enabled", &self.sync_enabled)
            .finish()
    }
}

impl SyncInput<SyncUnchecked> {
    #[must_use]
    pub const fn new(force: bool, sync_enabled: bool) -> Self {
        Self {
            force,
            sync_enabled,
            _state: PhantomData,
        }
    }

    #[must_use]
    pub const fn should_run(&self) -> bool {
        self.sync_enabled || self.force
    }

    #[must_use]
    pub const fn into_runnable(self) -> Option<SyncInput<SyncRunnable>> {
        if self.sync_enabled || self.force {
            Some(SyncInput {
                force: self.force,
                sync_enabled: self.sync_enabled,
                _state: PhantomData,
            })
        } else {
            None
        }
    }
}

impl SyncInput<SyncRunnable> {
    #[must_use]
    pub const fn force(&self) -> bool {
        self.force
    }

    #[must_use]
    pub const fn sync_enabled(&self) -> bool {
        self.sync_enabled
    }
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Skipped,
    Success(String),
    Failure(String),
}

#[derive(Debug, Clone)]
pub struct SyncOutput {
    pub status: SyncStatus,
}

#[derive(Debug, Clone)]
pub struct DoctorOutput {
    pub home: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub config_exists: bool,
    pub db_exists: bool,
    pub bookmark_count: i64,
}

#[derive(Debug, Clone)]
pub struct DoctorInput {
    pub home: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
}
