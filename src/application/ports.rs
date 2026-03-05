use anyhow::Result;

use crate::domain::{
    Ayah, AyahRef, Bookmark, BookmarkId, LanguageTag, SearchHit, SearchLimit, StudyProgress,
    SurahMeta, SurahNumber,
};

pub trait QuranReadRepository {
    fn list_surahs(&self) -> Result<Vec<SurahMeta>>;
    fn read_ayah(&self, target: AyahRef, lang: &LanguageTag) -> Result<Option<Ayah>>;
    fn read_surah(&self, surah: SurahNumber, lang: &LanguageTag) -> Result<Vec<Ayah>>;
    fn search(
        &self,
        query: &str,
        search_quran: bool,
        search_translation: bool,
        limit: SearchLimit,
    ) -> Result<Vec<SearchHit>>;
}

pub trait BookmarkRepository {
    fn add_bookmark(&self, target: AyahRef, note: Option<&str>) -> Result<BookmarkId>;
    fn list_bookmarks(&self) -> Result<Vec<Bookmark>>;
    fn remove_bookmark(&self, bookmark_id: BookmarkId) -> Result<usize>;
}

pub trait ProgressRepository {
    fn set_progress(&self, target: AyahRef) -> Result<()>;
    fn get_progress(&self) -> Result<Option<StudyProgress>>;
}

pub trait HealthRepository {
    fn bookmark_count(&self) -> Result<i64>;
}

pub trait SyncLogRepository {
    fn record_sync(&self, status: &str, message: &str) -> Result<()>;
}

pub trait SyncGateway {
    fn ping(&self) -> Result<()>;
}
