# tada-rust 📖

**Asisten Interaktif Belajar Al-Qur'an dari Terminal**

`tada-rust` adalah aplikasi terminal modern (TUI & CLI) untuk membaca, mendengarkan, dan mempelajari Al-Qur'an. Dibangun dengan Rust untuk performa tinggi, aplikasi ini menghadirkan pengalaman membaca Al-Qur'an yang nyaman langsung di terminal Anda dengan dukungan rendering teks Arab yang presisi.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/built_with-Rust-orange.svg)

## ✨ Fitur Utama

- **Terminal User Interface (TUI) Modern**: Antarmuka satu layar yang kaya fitur menggunakan `ratatui`.
- **Rendering Teks Arab Presisi**: Dukungan penuh untuk *reshaping* dan *bidirectional text* (RTL) agar teks Arab tampil benar di terminal.
- **Navigasi Cepat**: Pindah antar Surah dan Ayat dengan mudah menggunakan keyboard.
- **Audio Playback**: Dengarkan murottal per ayat atau per surah langsung dari aplikasi.
- **Pencarian Canggih**: Cari potongan ayat atau terjemahan dengan cepat.
- **Mode Wizard Interaktif**: Panduan langkah demi langkah untuk pemula.
- **Bookmark**: Simpan ayat favorit Anda untuk dibaca nanti.
- **Offline First**: Database dan aset disimpan lokal untuk akses cepat tanpa internet (kecuali streaming audio pertama kali).

## 🚀 Instalasi

Pastikan Anda telah menginstal [Rust & Cargo](https://rustup.rs/).

```bash
# Clone repository
git clone https://github.com/username/tada-rust.git
cd tada-rust

# Build dan jalankan
cargo run --release
```

### Persyaratan Font
Untuk pengalaman terbaik, gunakan terminal yang mendukung **Ligatures** dan karakter Arab dengan baik. Rekomendasi font:
- [Amiri](https://fonts.google.com/specimen/Amiri) (Sangat disarankan untuk teks Al-Qur'an)
- [Nerd Fonts](https://www.nerdfonts.com/) (Misal: JetBrains Mono Nerd Font)
- [Scheherazade New](https://software.sil.org/scheherazade/)

Jika teks Arab terlihat terputus-putus, pastikan terminal Anda menggunakan salah satu font di atas.

## 📖 Cara Penggunaan

### Mode TUI (Default)
Jalankan aplikasi tanpa argumen untuk masuk ke mode TUI:
```bash
cargo run
```

#### Kontrol Keyboard (TUI)
| Tombol | Fungsi |
|--------|--------|
| `j` / `↓` | Ayat Berikutnya / Kursor Bawah |
| `k` / `↑` | Ayat Sebelumnya / Kursor Atas |
| `n` | Surah Berikutnya |
| `p` | Surah Sebelumnya |
| `Spasi` | Play / Pause Audio |
| `[` / `]` | Audio Mundur / Maju |
| `s` | Stop Audio |
| `/` | Buka Pencarian |
| `f` | Tambah Bookmark |
| `Ctrl+b` | Toggle Sidebar Surah |
| `q` | Keluar Aplikasi |

### Mode CLI
Gunakan perintah CLI untuk operasi cepat atau scripting:

```bash
# Baca Surah Al-Fatihah (1)
tada-rust read --surah 1

# Cari kata "sabar" dalam terjemahan
tada-rust search "sabar"

# Masuk ke wizard interaktif
tada-rust interactive
```

## 🛠️ Pengembangan

Project ini menggunakan arsitektur *Clean Architecture* yang terbagi menjadi:
- `domain`: Core logic dan entity (Surah, Ayah, dll).
- `application`: Use cases dan business logic.
- `adapters`: Implementasi detail (Database, Audio, TUI).
- `interfaces`: Entry points (CLI, TUI).

### Menjalankan Test
```bash
cargo test
```

## 🤝 Kontribusi
Kontribusi sangat diterima! Silakan buat Issue atau Pull Request untuk fitur baru atau perbaikan bug.

## 📄 Lisensi
Project ini dilisensikan di bawah [MIT License](LICENSE).
