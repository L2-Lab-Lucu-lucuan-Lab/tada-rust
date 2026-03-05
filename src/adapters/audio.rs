use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use tokio::runtime::Builder;

use crate::domain::Ayah;

#[derive(Debug, Clone)]
pub struct AudioCache {
    root: PathBuf,
    enabled: bool,
    max_bytes: u64,
    client: Client,
}

impl AudioCache {
    pub fn new(root: impl AsRef<Path>, enabled: bool, max_mb: u64) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let client = Client::builder().build()?;
        Ok(Self {
            root,
            enabled,
            max_bytes: max_mb * 1024 * 1024,
            client,
        })
    }

    pub fn get_or_fetch_ayah(
        &self,
        ayah: &Ayah,
        selected_qari: Option<&str>,
        fallback_qari: &str,
    ) -> Result<Option<PathBuf>> {
        let qari = selected_qari.unwrap_or(fallback_qari);
        let url = match ayah.resolve_audio_url(selected_qari, fallback_qari) {
            Some(v) => v,
            None => return Ok(None),
        };
        self.get_or_fetch_url(&url, qari, ayah.surah_no, ayah.ayah_no, AudioKind::Ayah)
            .map(Some)
    }

    fn get_or_fetch_url(
        &self,
        url: &str,
        qari: &str,
        surah: u16,
        ayah: u16,
        kind: AudioKind,
    ) -> Result<PathBuf> {
        if !self.enabled {
            let temp_path = self.root.join("stream").join(format!(
                "{}-{:03}-{:03}.mp3",
                sanitize_qari_id(qari),
                surah,
                ayah
            ));
            if let Some(parent) = temp_path.parent() {
                fs::create_dir_all(parent)?;
            }
            self.download_to(url, &temp_path)?;
            return Ok(temp_path);
        }

        let kind_dir = match kind {
            AudioKind::Ayah => "ayah",
        };
        let path = self
            .root
            .join(kind_dir)
            .join(sanitize_qari_id(qari))
            .join(format!("{:03}-{:03}.mp3", surah, ayah));
        if path.exists() {
            return Ok(path);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.download_to(url, &path)?;
        self.enforce_limit()?;
        Ok(path)
    }

    fn download_to(&self, url: &str, path: &Path) -> Result<()> {
        let tmp = path.with_extension("mp3.tmp");
        let bytes = self.block_on(async {
            let resp = self.client.get(url).send().await?.error_for_status()?;
            let bytes = resp.bytes().await?;
            Ok::<_, anyhow::Error>(bytes.to_vec())
        })?;
        fs::write(&tmp, bytes).with_context(|| format!("Gagal menulis {}", tmp.display()))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("Gagal memindahkan cache ke {}", path.display()))?;
        Ok(())
    }

    fn enforce_limit(&self) -> Result<()> {
        if !self.enabled || self.max_bytes == 0 {
            return Ok(());
        }

        let mut files = Vec::new();
        collect_files(&self.root, &mut files)?;
        let mut total: u64 = 0;
        let mut entries = Vec::new();
        for path in files {
            let meta = fs::metadata(&path)?;
            let size = meta.len();
            total = total.saturating_add(size);
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((path, size, modified));
        }
        if total <= self.max_bytes {
            return Ok(());
        }

        entries.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in entries {
            if total <= self.max_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        Ok(())
    }

    fn block_on<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T, anyhow::Error>>,
    ) -> Result<T> {
        let rt = Builder::new_current_thread().enable_all().build()?;
        rt.block_on(fut)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioKind {
    Ayah,
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn sanitize_qari_id(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
}

pub struct AudioPlayer {
    stream: OutputStream,
    sink: Option<Sink>,
    ayahs: Vec<Ayah>,
    index: usize,
    cache: AudioCache,
    selected_qari: Option<String>,
    fallback_qari: String,
    playback_rate: f32,
    paused: bool,
    stopped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerTick {
    NoChange,
    AyahStarted(usize),
    Finished,
}

impl AudioPlayer {
    pub fn new(
        ayahs: Vec<Ayah>,
        start_index: usize,
        cache: AudioCache,
        selected_qari: Option<String>,
        fallback_qari: String,
    ) -> Result<Self> {
        if ayahs.is_empty() {
            return Err(anyhow!("Playlist audio kosong"));
        }
        let stream = OutputStreamBuilder::open_default_stream()?;
        let mut player = Self {
            stream,
            sink: None,
            ayahs,
            index: start_index,
            cache,
            selected_qari,
            fallback_qari,
            playback_rate: 1.0,
            paused: false,
            stopped: false,
        };
        let actual_start = player.index.min(player.ayahs.len().saturating_sub(1));
        player.start_at(actual_start)?;
        Ok(player)
    }

    pub fn current_ayah(&self) -> Option<&Ayah> {
        self.ayahs.get(self.index)
    }

    pub fn toggle_pause(&mut self) {
        if let Some(sink) = &self.sink {
            if self.paused {
                sink.play();
            } else {
                sink.pause();
            }
            self.paused = !self.paused;
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.sink = None;
        self.stopped = true;
    }

    pub fn advance(&mut self) -> Result<bool> {
        if self.index + 1 >= self.ayahs.len() {
            self.stop();
            return Ok(false);
        }
        self.start_at(self.index + 1)?;
        Ok(true)
    }

    pub fn prev(&mut self) -> Result<bool> {
        if self.index == 0 {
            return Ok(false);
        }
        self.start_at(self.index - 1)?;
        Ok(true)
    }

    pub fn restart_current(&mut self) -> Result<()> {
        self.start_at(self.index)
    }

    pub fn tick(&mut self) -> Result<PlayerTick> {
        if self.stopped {
            return Ok(PlayerTick::Finished);
        }

        if let Some(sink) = &self.sink
            && !self.paused
            && sink.empty()
        {
            if self.index + 1 < self.ayahs.len() {
                self.start_at(self.index + 1)?;
                return Ok(PlayerTick::AyahStarted(self.index));
            }
            self.stop();
            return Ok(PlayerTick::Finished);
        }
        Ok(PlayerTick::NoChange)
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn set_playback_rate(&mut self, rate: f32) {
        let clamped = rate.clamp(0.75, 1.25);
        self.playback_rate = clamped;
        if let Some(sink) = &self.sink {
            sink.set_speed(clamped);
        }
    }

    pub fn playback_rate(&self) -> f32 {
        self.playback_rate
    }

    pub fn set_qari(&mut self, qari_id: String) -> Result<()> {
        self.selected_qari = Some(qari_id);
        self.start_at(self.index)
    }

    fn start_at(&mut self, index: usize) -> Result<()> {
        self.index = index;
        self.paused = false;
        self.stopped = false;
        if let Some(sink) = &self.sink {
            sink.stop();
        }

        let ayah = self
            .ayahs
            .get(index)
            .ok_or_else(|| anyhow!("Index ayah di luar range"))?;
        let path = self
            .cache
            .get_or_fetch_ayah(ayah, self.selected_qari.as_deref(), &self.fallback_qari)?
            .ok_or_else(|| anyhow!("Audio tidak tersedia untuk ayat ini"))?;

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let source = Decoder::try_from(reader)?;
        let sink = Sink::connect_new(self.stream.mixer());
        sink.set_speed(self.playback_rate);
        sink.append(source);
        self.sink = Some(sink);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn _stream(&self) -> &OutputStream {
        &self.stream
    }
}

pub fn qari_name(id: &str) -> &'static str {
    match id {
        "01" => "Abdullah Al-Juhany",
        "02" => "Abdul Muhsin Al-Qasim",
        "03" => "Abdurrahman As-Sudais",
        "04" => "Ibrahim Al-Dossari",
        "05" => "Misyari Rasyid Al-Afasy",
        "06" => "Yasser Al-Dosari",
        _ => "Qari Tidak Dikenal",
    }
}
