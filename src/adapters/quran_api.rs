use std::collections::BTreeMap;
use std::future::Future;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde_json::{Value, json};
use tokio::runtime::Builder;

use crate::application::ports::{QuranReadRepository, SyncGateway};
use crate::domain::{Ayah, AyahRef, LanguageTag, SearchHit, SearchLimit, SurahMeta, SurahNumber};

const DEFAULT_QARI: &str = "05";

pub struct QuranApiClient {
    client: Client,
    base_url: String,
}

impl QuranApiClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
        Ok(Self {
            client,
            base_url: "https://equran.id/api".to_string(),
        })
    }

    fn block_on<T>(&self, fut: impl Future<Output = Result<T>>) -> Result<T> {
        let rt = Builder::new_current_thread().enable_all().build()?;
        rt.block_on(fut)
    }

    fn get_json(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        self.block_on(async {
            let response = self.client.get(url).send().await?;
            let response = response.error_for_status()?;
            let payload: Value = response.json().await?;
            Ok(payload)
        })
    }

    fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        self.block_on(async {
            let response = self.client.post(url).json(&body).send().await?;
            let response = response.error_for_status()?;
            let payload: Value = response.json().await?;
            Ok(payload)
        })
    }

    fn surah_detail(&self, surah: SurahNumber) -> Result<Value> {
        let payload = self.get_json(&format!("/v2/surat/{}", surah.value()))?;
        payload
            .get("data")
            .cloned()
            .ok_or_else(|| anyhow!("format response detail surah v2 tidak valid"))
    }

    fn audio_map(raw: &Value) -> BTreeMap<String, String> {
        raw.as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, value)| {
                        let url = value.as_str()?.trim();
                        (!url.is_empty()).then(|| (key.clone(), url.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn map_ayah(raw: &Value, surah_no: SurahNumber, lang: &LanguageTag) -> Option<Ayah> {
        let ayah_no = u16::try_from(raw.get("nomorAyat")?.as_u64()?).ok()?;
        let arabic_text = raw.get("teksArab")?.as_str()?.to_string();
        let transliteration = raw
            .get("teksLatin")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let translation = match lang.as_str() {
            "id" => raw
                .get("teksIndonesia")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            _ => raw
                .get("teksIndonesia")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        };

        let audio_urls = raw.get("audio").map(Self::audio_map).unwrap_or_default();
        let audio_url = audio_urls
            .get(DEFAULT_QARI)
            .cloned()
            .or_else(|| audio_urls.values().next().cloned());

        Some(Ayah {
            surah_no: surah_no.value(),
            ayah_no,
            arabic_text,
            transliteration,
            translation,
            audio_url,
            audio_urls,
        })
    }
}

impl SyncGateway for QuranApiClient {
    fn ping(&self) -> Result<()> {
        let _ = self.get_json("/v2/surat")?;
        Ok(())
    }
}

impl QuranReadRepository for QuranApiClient {
    fn list_surahs(&self) -> Result<Vec<SurahMeta>> {
        let payload = self.get_json("/v2/surat")?;
        let items = payload
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("format response list surah v2 tidak valid"))?;

        items
            .iter()
            .map(|item| {
                let surah_no_raw = item
                    .get("nomor")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow!("nomor surah tidak valid"))?;
                let surah_no = u16::try_from(surah_no_raw)
                    .with_context(|| format!("nomor surah overflow: {surah_no_raw}"))?;
                let name_ar = item
                    .get("nama")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let name_id = item
                    .get("namaLatin")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let ayah_count_raw = item
                    .get("jumlahAyat")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow!("jumlahAyat tidak valid"))?;
                let ayah_count = u16::try_from(ayah_count_raw)
                    .with_context(|| format!("jumlahAyat overflow: {ayah_count_raw}"))?;
                let audio_full_urls = item
                    .get("audioFull")
                    .map(Self::audio_map)
                    .unwrap_or_default();
                let audio_full = audio_full_urls
                    .get(DEFAULT_QARI)
                    .cloned()
                    .or_else(|| audio_full_urls.values().next().cloned());

                Ok(SurahMeta {
                    surah_no,
                    name_ar,
                    name_id,
                    ayah_count,
                    audio_full,
                    audio_full_urls,
                })
            })
            .collect()
    }

    fn read_ayah(&self, target: AyahRef, lang: &LanguageTag) -> Result<Option<Ayah>> {
        let detail = self.surah_detail(target.surah())?;
        let ayahs = detail
            .get("ayat")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("format ayat v2 tidak valid"))?;

        Ok(ayahs
            .iter()
            .find(|item| {
                item.get("nomorAyat").and_then(Value::as_u64)
                    == Some(u64::from(target.ayah().value()))
            })
            .and_then(|item| Self::map_ayah(item, target.surah(), lang)))
    }

    fn read_surah(&self, surah: SurahNumber, lang: &LanguageTag) -> Result<Vec<Ayah>> {
        let detail = self.surah_detail(surah)?;
        let ayahs = detail
            .get("ayat")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("format ayat v2 tidak valid"))?;

        Ok(ayahs
            .iter()
            .filter_map(|item| Self::map_ayah(item, surah, lang))
            .collect())
    }

    fn search(
        &self,
        query: &str,
        search_quran: bool,
        search_translation: bool,
        limit: SearchLimit,
    ) -> Result<Vec<SearchHit>> {
        let payload = self.post_json(
            "/vector",
            json!({
                "cari": query,
                "batas": limit.value(),
                "tipe": ["ayat"]
            }),
        )?;

        let items = payload
            .get("hasil")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("format response vector search tidak valid"))?;

        let max = usize::from(limit.value());
        Ok(items
            .iter()
            .filter_map(|item| {
                let data = item.get("data")?;
                let surah_no = u16::try_from(data.get("id_surat")?.as_u64()?).ok()?;
                let ayah_no = u16::try_from(data.get("nomor_ayat")?.as_u64()?).ok()?;

                let snippet = if search_quran && !search_translation {
                    data.get("teks_arab")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                } else {
                    data.get("terjemahan_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };

                (!snippet.is_empty()).then_some(SearchHit {
                    surah_no,
                    ayah_no,
                    snippet,
                })
            })
            .take(max)
            .collect())
    }
}
