use thiserror::Error;

use crate::application::dto::{
    BookmarkAddInput, ContinueInput, ContinueOutput, DoctorInput, DoctorOutput, PlanInput,
    PlanOutput, PlayInput, PlayOutput, ReadInput, ReadOutput, ReadTarget, SearchInput,
    SearchOutput, SearchScope, SyncInput, SyncOutput, SyncRunnable, SyncStatus,
};
use crate::application::ports::{
    BookmarkRepository, HealthRepository, ProgressRepository, QuranReadRepository, SyncGateway,
    SyncLogRepository,
};
use crate::domain::{
    AyahNumber, AyahRef, Bookmark, BookmarkId, DomainError, StudyProgress, SurahMeta, SurahNumber,
};

#[derive(Debug, Error)]
pub enum UseCaseError {
    #[error("ayat tidak ditemukan: {0}")]
    AyahNotFound(AyahRef),
    #[error("surah tidak ditemukan: {0}")]
    SurahNotFound(SurahNumber),
    #[error("belum ada progress. gunakan `read` atau `tui` dulu.")]
    MissingProgress,
    #[error("progress terakhir tidak ditemukan di API: {0}")]
    ProgressNotFound(AyahRef),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Infra(#[from] anyhow::Error),
}

pub type UseCaseResult<T> = std::result::Result<T, UseCaseError>;

pub fn read_quran<R, P>(
    read_repo: &R,
    progress_repo: &P,
    input: ReadInput,
) -> UseCaseResult<ReadOutput>
where
    R: QuranReadRepository,
    P: ProgressRepository,
{
    match input.target {
        ReadTarget::Ayah(target) => {
            let ayah = read_repo
                .read_ayah(target, &input.lang)?
                .ok_or(UseCaseError::AyahNotFound(target))?;
            progress_repo.set_progress(ayah_ref_from_raw(ayah.surah_no, ayah.ayah_no)?)?;
            Ok(ReadOutput::Single(ayah))
        }
        ReadTarget::Surah(surah) => {
            let ayahs = read_repo.read_surah(surah, &input.lang)?;
            if ayahs.is_empty() {
                return Err(UseCaseError::SurahNotFound(surah));
            }

            if let Some(last) = ayahs.last() {
                progress_repo.set_progress(ayah_ref_from_raw(last.surah_no, last.ayah_no)?)?;
            }
            Ok(ReadOutput::Surah(ayahs))
        }
    }
}

pub fn search_quran<R>(repo: &R, input: SearchInput) -> UseCaseResult<SearchOutput>
where
    R: QuranReadRepository,
{
    let (search_quran, search_translation) = match input.scope {
        SearchScope::Quran => (true, false),
        SearchScope::Translation => (false, true),
        SearchScope::All => (true, true),
    };

    let hits = repo.search(&input.query, search_quran, search_translation, input.limit)?;
    Ok(SearchOutput { hits })
}

pub fn prepare_play<R>(repo: &R, input: PlayInput) -> UseCaseResult<PlayOutput>
where
    R: QuranReadRepository,
{
    let selected_qari = input.qari.clone();

    match input.target {
        ReadTarget::Ayah(target) => {
            let ayah = repo
                .read_ayah(target, &input.lang)?
                .ok_or(UseCaseError::AyahNotFound(target))?;
            let target_audio = ayah.resolve_audio_url(
                selected_qari.as_ref().map(|q| q.as_str()),
                input.fallback_qari.as_str(),
            );
            Ok(PlayOutput {
                ayahs: vec![ayah],
                target_audio,
                selected_qari,
            })
        }
        ReadTarget::Surah(surah) => {
            let ayahs = repo.read_surah(surah, &input.lang)?;
            if ayahs.is_empty() {
                return Err(UseCaseError::SurahNotFound(surah));
            }

            let target_audio = repo
                .list_surahs()?
                .into_iter()
                .find(|item| item.surah_no == surah.value())
                .and_then(|item| {
                    item.resolve_audio_full_url(
                        selected_qari.as_ref().map(|q| q.as_str()),
                        input.fallback_qari.as_str(),
                    )
                });

            Ok(PlayOutput {
                ayahs,
                target_audio,
                selected_qari,
            })
        }
    }
}

pub fn add_bookmark<R>(repo: &R, input: BookmarkAddInput) -> UseCaseResult<BookmarkId>
where
    R: BookmarkRepository,
{
    Ok(repo.add_bookmark(input.target, input.note.as_deref())?)
}

pub fn list_bookmarks<R>(repo: &R) -> UseCaseResult<Vec<Bookmark>>
where
    R: BookmarkRepository,
{
    Ok(repo.list_bookmarks()?)
}

pub fn remove_bookmark<R>(repo: &R, bookmark_id: BookmarkId) -> UseCaseResult<usize>
where
    R: BookmarkRepository,
{
    Ok(repo.remove_bookmark(bookmark_id)?)
}

pub fn continue_reading<R, P>(
    read_repo: &R,
    progress_repo: &P,
    input: ContinueInput,
) -> UseCaseResult<ContinueOutput>
where
    R: QuranReadRepository,
    P: ProgressRepository,
{
    let progress = progress_repo
        .get_progress()?
        .ok_or(UseCaseError::MissingProgress)?;
    let target = ayah_ref_from_progress(progress)?;
    let ayah = read_repo
        .read_ayah(target, &input.lang)?
        .ok_or(UseCaseError::ProgressNotFound(target))?;
    Ok(ContinueOutput { ayah })
}

pub fn daily_plan<R, P>(
    read_repo: &R,
    progress_repo: &P,
    input: PlanInput,
) -> UseCaseResult<PlanOutput>
where
    R: QuranReadRepository,
    P: ProgressRepository,
{
    let surahs = read_repo.list_surahs()?;
    if surahs.is_empty() {
        return Ok(PlanOutput { ayahs: Vec::new() });
    }

    let initial_position = match progress_repo.get_progress()? {
        Some(progress) => ayah_ref_from_progress(progress)?,
        None => first_position(&surahs)?,
    };

    let mut out = Vec::new();
    let mut cursor = initial_position;
    let mut guard = 0_usize;
    while out.len() < input.count.value() && guard < 10_000 {
        guard += 1;

        if let Some(ayah) = read_repo.read_ayah(cursor, &input.lang)? {
            out.push(ayah);
        }

        if let Some(next) = next_position(&surahs, cursor)? {
            cursor = next;
        } else {
            break;
        }
    }

    Ok(PlanOutput { ayahs: out })
}

fn first_position(surahs: &[SurahMeta]) -> UseCaseResult<AyahRef> {
    let first = surahs
        .first()
        .ok_or_else(|| UseCaseError::Infra(anyhow::anyhow!("empty surah list")))?;
    let surah = SurahNumber::new(first.surah_no)?;
    let ayah = AyahNumber::new(1)?;
    Ok(AyahRef::new(surah, ayah))
}

fn next_position(surahs: &[SurahMeta], position: AyahRef) -> UseCaseResult<Option<AyahRef>> {
    let idx = surahs
        .iter()
        .position(|s| s.surah_no == position.surah().value())
        .ok_or_else(|| {
            UseCaseError::Infra(anyhow::anyhow!(
                "surah {} missing from metadata",
                position.surah()
            ))
        })?;
    let current = &surahs[idx];
    if position.ayah().value() < current.ayah_count {
        return Ok(Some(AyahRef::new(
            position.surah(),
            AyahNumber::new(position.ayah().value() + 1)?,
        )));
    }

    if idx + 1 < surahs.len() {
        let next_surah = SurahNumber::new(surahs[idx + 1].surah_no)?;
        return Ok(Some(AyahRef::new(next_surah, AyahNumber::new(1)?)));
    }
    Ok(None)
}

fn ayah_ref_from_raw(surah_no: u16, ayah_no: u16) -> UseCaseResult<AyahRef> {
    Ok(AyahRef::new(
        SurahNumber::new(surah_no)?,
        AyahNumber::new(ayah_no)?,
    ))
}

fn ayah_ref_from_progress(progress: StudyProgress) -> UseCaseResult<AyahRef> {
    ayah_ref_from_raw(progress.last_surah_no, progress.last_ayah_no)
}

pub fn doctor_report<R>(repo: &R, input: DoctorInput) -> UseCaseResult<DoctorOutput>
where
    R: HealthRepository,
{
    Ok(DoctorOutput {
        home: input.home.clone(),
        config_path: input.config_path.clone(),
        db_path: input.db_path.clone(),
        config_exists: input.config_path.exists(),
        db_exists: input.db_path.exists(),
        bookmark_count: repo.bookmark_count()?,
    })
}

pub fn sync_content<R, G>(
    repo: &R,
    gateway: &G,
    input: SyncInput<SyncRunnable>,
) -> UseCaseResult<SyncOutput>
where
    R: SyncLogRepository,
    G: SyncGateway,
{
    let _ = (input.force(), input.sync_enabled());
    match gateway.ping() {
        Ok(()) => {
            repo.record_sync("ok", "Berhasil menghubungi equran.id API v2")?;
            Ok(SyncOutput {
                status: SyncStatus::Success("Sync metadata berhasil".to_string()),
            })
        }
        Err(err) => {
            let message = format!("Sync gagal: {err}");
            repo.record_sync("error", &message)?;
            Ok(SyncOutput {
                status: SyncStatus::Failure(message),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::Result as AnyResult;
    use anyhow::{Result, anyhow};

    use super::*;
    use crate::application::ports::{
        BookmarkRepository, HealthRepository, ProgressRepository, QuranReadRepository, SyncGateway,
        SyncLogRepository,
    };
    use crate::domain::{Ayah, LanguageTag, QariId, SearchHit, SearchLimit, StudyProgress};

    #[derive(Default)]
    struct MockRepo {
        logs: Mutex<Vec<(String, String)>>,
    }

    impl QuranReadRepository for MockRepo {
        fn list_surahs(&self) -> AnyResult<Vec<SurahMeta>> {
            Ok(vec![
                SurahMeta {
                    surah_no: 1,
                    name_ar: "Al-Fatihah".to_string(),
                    name_id: "Al-Fatihah".to_string(),
                    ayah_count: 7,
                    audio_full: Some("https://cdn.equran.id/audio-full/sample/001.mp3".to_string()),
                    audio_full_urls: std::collections::BTreeMap::from([(
                        "05".to_string(),
                        "https://cdn.equran.id/audio-full/sample/001.mp3".to_string(),
                    )]),
                },
                SurahMeta {
                    surah_no: 2,
                    name_ar: "Al-Baqarah".to_string(),
                    name_id: "Al-Baqarah".to_string(),
                    ayah_count: 286,
                    audio_full: Some("https://cdn.equran.id/audio-full/sample/002.mp3".to_string()),
                    audio_full_urls: std::collections::BTreeMap::from([(
                        "05".to_string(),
                        "https://cdn.equran.id/audio-full/sample/002.mp3".to_string(),
                    )]),
                },
            ])
        }

        fn read_ayah(&self, target: AyahRef, _lang: &LanguageTag) -> AnyResult<Option<Ayah>> {
            Ok(Some(Ayah {
                surah_no: target.surah().value(),
                ayah_no: target.ayah().value(),
                arabic_text: "sample".to_string(),
                transliteration: None,
                translation: Some("sample".to_string()),
                audio_url: Some("https://cdn.equran.id/audio-partial/sample.mp3".to_string()),
                audio_urls: std::collections::BTreeMap::from([(
                    "05".to_string(),
                    "https://cdn.equran.id/audio-partial/sample.mp3".to_string(),
                )]),
            }))
        }

        fn read_surah(&self, surah: SurahNumber, _lang: &LanguageTag) -> AnyResult<Vec<Ayah>> {
            Ok(vec![Ayah {
                surah_no: surah.value(),
                ayah_no: 1,
                arabic_text: "sample".to_string(),
                transliteration: None,
                translation: Some("sample".to_string()),
                audio_url: Some("https://cdn.equran.id/audio-partial/sample.mp3".to_string()),
                audio_urls: std::collections::BTreeMap::from([(
                    "05".to_string(),
                    "https://cdn.equran.id/audio-partial/sample.mp3".to_string(),
                )]),
            }])
        }

        fn search(
            &self,
            _query: &str,
            _search_quran: bool,
            _search_translation: bool,
            _limit: SearchLimit,
        ) -> AnyResult<Vec<SearchHit>> {
            Ok(vec![SearchHit {
                surah_no: 1,
                ayah_no: 1,
                snippet: "sample".to_string(),
            }])
        }
    }

    impl BookmarkRepository for MockRepo {
        fn add_bookmark(&self, _target: AyahRef, _note: Option<&str>) -> AnyResult<BookmarkId> {
            Ok(BookmarkId::new(1)?)
        }

        fn list_bookmarks(&self) -> AnyResult<Vec<crate::domain::Bookmark>> {
            Ok(vec![])
        }

        fn remove_bookmark(&self, _bookmark_id: BookmarkId) -> AnyResult<usize> {
            Ok(1)
        }
    }

    impl ProgressRepository for MockRepo {
        fn set_progress(&self, _target: AyahRef) -> AnyResult<()> {
            Ok(())
        }

        fn get_progress(&self) -> AnyResult<Option<StudyProgress>> {
            Ok(Some(StudyProgress {
                last_surah_no: 1,
                last_ayah_no: 1,
            }))
        }
    }

    impl HealthRepository for MockRepo {
        fn bookmark_count(&self) -> AnyResult<i64> {
            Ok(2)
        }
    }

    impl SyncLogRepository for MockRepo {
        fn record_sync(&self, status: &str, message: &str) -> AnyResult<()> {
            let mut logs = self
                .logs
                .lock()
                .map_err(|err| anyhow!("lock poisoned: {err}"))?;
            logs.push((status.to_string(), message.to_string()));
            Ok(())
        }
    }

    struct MockGatewayOk;
    struct MockGatewayErr;

    impl SyncGateway for MockGatewayOk {
        fn ping(&self) -> AnyResult<()> {
            Ok(())
        }
    }

    impl SyncGateway for MockGatewayErr {
        fn ping(&self) -> AnyResult<()> {
            Err(anyhow!("offline"))
        }
    }

    #[test]
    fn read_single_updates_happy_path() -> Result<()> {
        let repo = MockRepo::default();
        let out = read_quran(
            &repo,
            &repo,
            ReadInput::for_ayah(
                AyahRef::new(SurahNumber::new(1)?, AyahNumber::new(1)?),
                LanguageTag::new("id")?,
            ),
        )?;
        assert!(matches!(out, ReadOutput::Single(_)));
        Ok(())
    }

    #[test]
    fn daily_plan_uses_api_repo() -> Result<()> {
        let repo = MockRepo::default();
        let out = daily_plan(
            &repo,
            &repo,
            PlanInput {
                count: crate::domain::PlanCount::new(3)?,
                lang: LanguageTag::new("id")?,
            },
        )?;
        assert_eq!(out.ayahs.len(), 3);
        Ok(())
    }

    #[test]
    fn prepare_play_returns_audio_url() -> Result<()> {
        let repo = MockRepo::default();
        let out = prepare_play(
            &repo,
            PlayInput {
                target: ReadTarget::Ayah(AyahRef::new(SurahNumber::new(1)?, AyahNumber::new(1)?)),
                lang: LanguageTag::new("id")?,
                qari: Some(QariId::new("05")?),
                fallback_qari: QariId::new("05")?,
            },
        )?;
        assert!(out.target_audio.is_some());
        Ok(())
    }

    #[test]
    fn sync_skips_when_disabled() -> Result<()> {
        let repo = MockRepo::default();
        let gateway = MockGatewayOk;
        let input = SyncInput::new(false, false);
        assert!(input.into_runnable().is_none());

        let runnable = SyncInput::new(false, true)
            .into_runnable()
            .ok_or_else(|| anyhow!("sync input should be runnable"))?;
        let out = sync_content(&repo, &gateway, runnable)?;

        assert!(matches!(out.status, SyncStatus::Success(_)));
        Ok(())
    }

    #[test]
    fn sync_records_failure() -> Result<()> {
        let repo = MockRepo::default();
        let gateway = MockGatewayErr;
        let runnable = SyncInput::new(true, true)
            .into_runnable()
            .ok_or_else(|| anyhow!("sync input should be runnable"))?;
        let out = sync_content(&repo, &gateway, runnable)?;

        assert!(matches!(out.status, SyncStatus::Failure(_)));
        Ok(())
    }
}
