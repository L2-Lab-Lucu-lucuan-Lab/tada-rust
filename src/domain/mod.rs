use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("surah must be between 1 and 114, got {0}")]
    InvalidSurah(u16),
    #[error("ayah must be between 1 and 286, got {0}")]
    InvalidAyah(u16),
    #[error("bookmark id must be > 0, got {0}")]
    InvalidBookmarkId(i64),
    #[error("unsupported qari id: {0}")]
    InvalidQari(String),
    #[error("language tag cannot be empty")]
    EmptyLanguageTag,
    #[error("language tag contains invalid characters: {0}")]
    InvalidLanguageTag(String),
    #[error("search limit must be between 1 and 200, got {0}")]
    InvalidSearchLimit(u16),
    #[error("plan count must be between 1 and 1000, got {0}")]
    InvalidPlanCount(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurahNumber(u16);

impl SurahNumber {
    pub const MIN: u16 = 1;
    pub const MAX: u16 = 114;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidSurah(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for SurahNumber {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SurahNumber> for u16 {
    fn from(value: SurahNumber) -> Self {
        value.value()
    }
}

impl Display for SurahNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AyahNumber(u16);

impl AyahNumber {
    pub const MIN: u16 = 1;
    pub const MAX: u16 = 286;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidAyah(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for AyahNumber {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AyahNumber> for u16 {
    fn from(value: AyahNumber) -> Self {
        value.value()
    }
}

impl Display for AyahNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BookmarkId(i64);

impl BookmarkId {
    pub fn new(value: i64) -> Result<Self, DomainError> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidBookmarkId(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for BookmarkId {
    type Error = DomainError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<BookmarkId> for i64 {
    fn from(value: BookmarkId) -> Self {
        value.value()
    }
}

impl Display for BookmarkId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QariId(String);

impl QariId {
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let canonical = normalize_qari_id(value)
            .ok_or_else(|| DomainError::InvalidQari(value.trim().to_string()))?;
        Ok(Self(canonical.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for QariId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl TryFrom<&str> for QariId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for QariId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageTag(String);

impl LanguageTag {
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(DomainError::EmptyLanguageTag);
        }
        let is_valid = normalized
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '-');
        if !is_valid {
            return Err(DomainError::InvalidLanguageTag(normalized.to_string()));
        }
        Ok(Self(normalized.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for LanguageTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl TryFrom<&str> for LanguageTag {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for LanguageTag {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchLimit(u16);

impl SearchLimit {
    pub const MIN: u16 = 1;
    pub const MAX: u16 = 200;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidSearchLimit(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for SearchLimit {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SearchLimit> for u16 {
    fn from(value: SearchLimit) -> Self {
        value.value()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanCount(usize);

impl PlanCount {
    pub const MIN: usize = 1;
    pub const MAX: usize = 1000;

    pub fn new(value: usize) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidPlanCount(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for PlanCount {
    type Error = DomainError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PlanCount> for usize {
    fn from(value: PlanCount) -> Self {
        value.value()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AyahRef {
    surah: SurahNumber,
    ayah: AyahNumber,
}

impl AyahRef {
    pub const fn new(surah: SurahNumber, ayah: AyahNumber) -> Self {
        Self { surah, ayah }
    }

    #[must_use]
    pub const fn surah(self) -> SurahNumber {
        self.surah
    }

    #[must_use]
    pub const fn ayah(self) -> AyahNumber {
        self.ayah
    }
}

impl Display for AyahRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.surah, self.ayah)
    }
}

#[derive(Debug, Clone)]
pub struct SurahMeta {
    pub surah_no: u16,
    pub name_ar: String,
    pub name_id: String,
    pub ayah_count: u16,
    pub audio_full: Option<String>,
    pub audio_full_urls: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Ayah {
    pub surah_no: u16,
    pub ayah_no: u16,
    pub arabic_text: String,
    pub transliteration: Option<String>,
    pub translation: Option<String>,
    pub audio_url: Option<String>,
    pub audio_urls: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub surah_no: u16,
    pub ayah_no: u16,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: i64,
    pub surah_no: u16,
    pub ayah_no: u16,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct StudyProgress {
    pub last_surah_no: u16,
    pub last_ayah_no: u16,
}

impl Ayah {
    pub fn resolve_audio_url(
        &self,
        selected_qari: Option<&str>,
        fallback_qari: &str,
    ) -> Option<String> {
        resolve_qari_url(
            &self.audio_urls,
            selected_qari,
            Some(fallback_qari),
            self.audio_url.as_deref(),
        )
    }
}

impl SurahMeta {
    pub fn resolve_audio_full_url(
        &self,
        selected_qari: Option<&str>,
        fallback_qari: &str,
    ) -> Option<String> {
        resolve_qari_url(
            &self.audio_full_urls,
            selected_qari,
            Some(fallback_qari),
            self.audio_full.as_deref(),
        )
    }
}

fn resolve_qari_url(
    urls: &BTreeMap<String, String>,
    selected_qari: Option<&str>,
    fallback_qari: Option<&str>,
    default_url: Option<&str>,
) -> Option<String> {
    if let Some(qari) = selected_qari
        && let Some(url) = urls.get(qari)
    {
        return Some(url.clone());
    }

    if let Some(qari) = fallback_qari
        && let Some(url) = urls.get(qari)
    {
        return Some(url.clone());
    }

    if let Some((_, url)) = urls.iter().next() {
        return Some(url.clone());
    }

    default_url.map(ToString::to_string)
}

#[must_use]
pub fn normalize_qari_id(input: &str) -> Option<&'static str> {
    match input.trim().to_ascii_lowercase().as_str() {
        "01" | "1" | "abdullah" | "juhany" => Some("01"),
        "02" | "2" | "qasim" | "abdulmuhsin" => Some("02"),
        "03" | "3" | "sudais" | "abdurrahman" => Some("03"),
        "04" | "4" | "dossari" | "ibrahim" => Some("04"),
        "05" | "5" | "afasy" | "misyari" => Some("05"),
        "06" | "6" | "yasser" | "dosari" => Some("06"),
        _ => None,
    }
}
