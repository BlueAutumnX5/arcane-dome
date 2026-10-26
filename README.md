![English](https://img.shields.io/badge/README-English-1f6feb?style=for-the-badge)
## 🇬🇧 ENGLISH Section ||| Untuk Bahasa Indonesia, scroll ke bawah lagi.
>  [!CAUTION]
> # VERY VERY VERY VERY EARLY development!1!1!1! Do not expect playable visuals and gameplay yet!

# Arcane Dome

A 2D side-scrolling platform arena shooter written in Rust with
[Macroquad](https://macroquad.rs). Fast arena matches against AI bots, with
weapons, maps and modes all defined in JSON.

> Status: early development. See [SPEC.md](SPEC.md) for the design and milestones.

## Whats already in the game

- **3 maps**:
  - Foundry Ascent: a steel mill around a lava pit, with a climbable tower and
    a thruster shaft. 2 to 4 teams.
  - Frostline Relay: a frozen mountain relay station built for 3 and 4 teams,
    with cliffs, a valley, an ice island and a relay shaft to the tower roof.
  - Redstone Arches: a desert canyon built for 2 teams, under a giant stone
    arch you can walk across.
- **2 modes**, Deathmatch and Elimination, each played free-for-all or with
  2 to 4 teams (Blue, Red, Green, Yellow; each team picks its colour).
  Elimination comes in three styles: Rounds, Last Man Standing (everyone has
  their own lives) and Attrition (each team shares a pool of lives). Every
  limit can be turned off, even all of them for an endless match. Every
  match and round starts with a 3 second countdown.
- **10 weapons**: Pistol, Assault Rifle, Leech SMG (heals you as you hit),
  Shotgun, Flamethrower, Marksman Rifle, Sniper Rifle, Rocket Launcher,
  Grenade Launcher and Laser.
- **Bots** with a view cone, reaction time and three difficulties (Easy,
  Normal, Hard), set per team.
- **Map gadgets**: thrusters (stand on the pad and press jump), weapon
  stations, shield stations, and powerup spots (Divine Shield, Rapid Fire,
  Critical Strike, Shadowstep).
- **Map editor**: draw terrain, place spawns, pickups and gadgets, undo and
  redo, and set the map's name, size, teams and match types. Validate checks
  that bots can reach everything, Test Play lets you play the map against bots
  right away, and Publish puts it into Quick Match. Export and Import share
  maps through the "Arcane Dome Maps" folder in Documents (or drop a map file
  onto the editor). Every button explains itself when you rest the mouse on
  it, and Instructions (F1) covers the rest.
- **Settings**: language, fullscreen, menu slides, blood effects and an FPS
  counter, remembered between runs. View Changes on the main menu shows what
  is new.
- **Languages**: English and Bahasa Indonesia (the map editor is English
  only for now).
- **Android** (9 and newer): the same game with touch controls. The map
  editor stays on the PC version for now.
- Placeholder art: shapes and colours drawn in code. No sound yet.

## Controls

| Action | Keys |
|---|---|
| Move | A / D (or arrow keys) |
| Jump, double jump | W, Space or Up (press again in the air) |
| Crouch | Hold S |
| Drop through a hatch | Press S while standing on it |
| Aim and fire | Mouse, left button |
| Reload | R |
| Switch weapon | 1 to 0, Q / E, or the mouse wheel |
| Use Shadowstep | F |
| Scoreboard | Hold Tab |
| Pause | P or Esc |
| Fullscreen | F11 |

On Android, two thumbs do it all:

| Action | Touch |
|---|---|
| Move | Left stick, left / right |
| Jump, double jump | Jump (tap again in the air) |
| Crouch, drop through a hatch | Push the left stick down |
| Fire | Hold Fire: it aims at the nearest enemy you can see |
| Aim yourself | Drag away from Fire, in the direction to shoot |
| Reload, switch weapon | The buttons above Fire |
| Pick a weapon | Tap it in the bar at the bottom |
| Use Shadowstep | Step (shows while you hold one) |
| Pause, scoreboard | Top left buttons (Back also pauses) |

The stick floats: put your thumb down anywhere on the left side.

## Inspiration and credits

Arcane Dome is a fan-made, original game **inspired by the _Raze_ Flash game series
by JuiceTin and AddisonR**. Thank you for the countless hours of fun.

This project is **not affiliated with or endorsed by** the original authors. It
contains no art, audio, code, names or map layouts from the original games. All
code and content here are original or used under open licenses listed in
[CREDITS.md](CREDITS.md).

## Building

Requires a stable Rust toolchain.

```bash
cargo run --release
```

```bash
cargo test
```

On Linux, Macroquad needs the usual system libraries: ALSA, X11, Xi and an
OpenGL driver (for example `alsa-lib libx11 libxi mesa` on Arch).

### Android

`android/build-apk.sh` builds `target/android/arcane-dome-<version>.apk`. It
needs the Android SDK (`ANDROID_HOME`) with an NDK, build-tools and a
platform, a JDK, and the Rust targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
```

```bash
ANDROID_HOME=~/Android/Sdk android/build-apk.sh
```

Without a release key it signs with the standard Android debug key, which is
fine for trying it on your own phone. `ARCANE_TOUCH=1` shows the touch
controls on the desktop, to check their layout.

## Adding content

- Translations: `assets/lang/id.json` maps each English text to its
  Bahasa Indonesia version (`{}` marks a number or name). `cargo test` lists
  anything missing.
- Weapons: one JSON file per weapon in `assets/data/weapons/`
- Maps: one JSON file per map in `assets/data/maps/`
- Powerups: `assets/data/powerups.json`

No code changes are needed to add a weapon or a map. Schemas are in [SPEC.md](SPEC.md).

### Maps

Maps are hand-designed out of freeform polygons (`shapes`). Each shape has a
`kind`:

- `solid`: blocks bodies and bullets from every side
- `hatch`: a thin floor you can stand on and drop through, and that thruster
  riders fly up through from below
- `water`: slows you down and lets you swim
- `hazard`: hurts anything inside it (`dps`), for lava, spikes or a bottomless void

A `material` (rock, metal, wood, sand, moss, sludge, lava...) picks the look.
All positions are in tiles. Spawns, pickups and objects are given by their
**feet** position (the point where a fighter would stand).

Objects are thrusters (`height_tiles`), weapon stations, shield stations
(`shield` 50 or 100) and powerup spots (`powerup`: always the same powerup).
Purely visual `decor` (walls, windows, girders, chains, lamps, trees...) and a
`theme` (sky colours, sun, skyline) set the mood.

Spawns belong to a team (0 to 3; the player is on team 0) or to free-for-all, and can change
owner with the number of teams (`by_teams`). A map
lists which team counts it is fair for with `team_counts`, for example `[2, 4]`.

Check that every spawn, pickup and object can be reached by bots:

```bash
cargo run --release --example map_check
```

```bash
cargo run --release --example map_check -- foundry_ascent nav.json
```

The second form checks one map and also writes its navigation graph as JSON.
Unfinished maps live in `drafts/maps/` and are not loaded by the game.

To watch bots play a map and report anyone stuck or falling into hazards (add
`MODE=elimination TEAMS=4` for team matches, `VERBOSE=1` for details):

```bash
cargo run --release --example bot_watch -- foundry_ascent 180 4
```

## Roadmap

Milestone 1 is done (v0.1.0). Milestone 2:

1. A map editor, so anyone can build and share maps (done)
2. Animations and textures
3. Sound
4. Possibly Capture the Flag and King of the Hill

## License

Code is released under the [MIT License](LICENSE). Third-party assets keep their
own licenses, listed in [CREDITS.md](CREDITS.md).

<br>

---
---

<br>

![Bahasa Indonesia](https://img.shields.io/badge/README-Bahasa_Indonesia-dc2626?style=for-the-badge)
## 🇮🇩 Bahasa Indonesia
> [!CAUTION]
> # PENGEMBANGAN MASIH SANGAT SANGAT SANGAT AWAL!1!1!1! Jangan berharap visual dan gameplay sudah siap dimainkan saat ini!

# Arcane Dome

Game tembak-menembak arena platformer 2 Dimensi, side-scrolling, yang dibuat menggunakan Rust dan [Macroquad](https://macroquad.rs). Bertarunglah dalam pertandingan arena singkat melawan bot AI, dengan senjata, peta, dan mode permainan yang semuanya didefinisikan dalam JSON.

> Status: masih dalam tahap pengembangan awal. Lihat [SPEC.md](SPEC.md) untuk mengetahui rancangan dan tahapan pengembangan.

## Apa saja yang sudah ada di dalam game?

- **3 peta:**
  - **Foundry Ascent:** pabrik baja di sekitar lubang lava, dilengkapi menara yang bisa dipanjat dan lorong pendorong (*thruster shaft*). Mendukung 2 hingga 4 tim.
  - **Frostline Relay:** stasiun pemancar di pegunungan bersalju yang dirancang untuk 3 dan 4 tim, dengan tebing, lembah, pulau es, dan lorong pemancar menuju atap menara.
  - **Redstone Arches:** ngarai gurun untuk 2 tim, dengan sebuah lengkungan batu raksasa yang bisa dilintasi.
- **2 mode permainan:** Deathmatch dan Elimination. Keduanya bisa dimainkan secara *free-for-all* atau dengan 2 hingga 4 tim (Biru, Merah, Hijau, dan Kuning; setiap tim memilih warnanya sendiri). Elimination memiliki tiga variasi: **Rounds** (pertandingan berbasis ronde), **Last Man Standing** (setiap pemain memiliki jatah nyawa sendiri), dan **Attrition** (setiap tim berbagi satu jatah nyawa). Semua batas pertandingan bisa dinonaktifkan, termasuk semuanya sekaligus untuk menghasilkan pertandingan tanpa akhir. Setiap pertandingan dan ronde dimulai dengan hitung mundur 3 detik.
- **10 senjata:** Pistol, Assault Rifle, Leech SMG (memulihkan kesehatan saat peluru mengenai lawan), Shotgun, Flamethrower, Marksman Rifle, Sniper Rifle, Rocket Launcher, Grenade Launcher, dan Laser.
- **Bot AI** dengan kerucut pandang, waktu reaksi, dan tiga tingkat kesulitan (Easy, Normal, Hard). Pengaturan dapat ditentukan untuk setiap tim.
- **Perangkat khusus pada peta:** pendorong (*thruster*, injak landasannya lalu tekan tombol lompat), stasiun senjata, stasiun perisai, dan titik kemunculan *power-up* (Divine Shield, Rapid Fire, Critical Strike, Shadowstep).
- **Editor peta:** gambar medan, tempatkan titik kemunculan pemain, item, dan perangkat khusus, batalkan atau ulangi perubahan, serta atur nama, ukuran, jumlah tim, dan jenis pertandingan untuk peta tersebut. Fitur **Validate** memeriksa apakah bot bisa mencapai semua lokasi penting. **Test Play** memungkinkanmu langsung mencoba peta melawan bot, sedangkan **Publish** memasukkan peta ke Quick Match. Fitur Export dan Import memungkinkan berbagi peta melalui folder `Arcane Dome Maps` di dalam Documents. Kamu juga bisa menyeret berkas peta ke editor. Setiap tombol memiliki penjelasan yang muncul saat kursor diarahkan ke atasnya, sedangkan menu Instructions (F1) menjelaskan hal-hal lainnya.
- **Pengaturan:** bahasa, mode layar penuh, animasi transisi antarmenu, efek darah, dan penghitung FPS. Pengaturan akan diingat ketika game dijalankan kembali. Menu utama juga menyediakan **View Changes** untuk melihat perubahan terbaru.
- **Bahasa:** English dan Bahasa Indonesia. Untuk saat ini, editor peta hanya tersedia dalam bahasa Inggris.
- **Android (versi 9 dan lebih baru):** game yang sama dengan kontrol layar sentuh. Editor peta masih tersedia di versi PC saja.
- Grafis sementara (*placeholder art*): bentuk dan warna yang digambar langsung melalui kode. Belum ada suara.

## Kontrol

### PC

| Aksi | Tombol |
|---|---|
| Bergerak | A / D atau tombol panah |
| Lompat dan double jump | W, Space, atau panah Atas (tekan sekali lagi saat berada di udara) |
| Merunduk | Tahan S |
| Turun menembus lantai jebakan | Tekan S saat berdiri di atasnya |
| Membidik dan menembak | Mouse, tombol kiri |
| Isi ulang amunisi/Reload | R |
| Mengganti senjata | Tombol 1 sampai 0, Q / E, atau roda mouse |
| Menggunakan Shadowstep | F |
| Papan skor | Tahan Tab |
| Jeda permainan | P atau Esc |
| Layar penuh | F11 |

### Android

Dua ibu jari sudah cukup untuk mengendalikan semuanya.

| Aksi | Kontrol sentuh |
|---|---|
| Bergerak | Joystick kiri, ke kiri atau kanan |
| Lompat dan double jump | Tombol Jump (ketuk lagi saat berada di udara) |
| Merunduk atau turun menembus lantai jebakan | Dorong joystick kiri ke bawah |
| Menembak | Tahan Fire. Karakter akan membidik musuh terdekat yang terlihat |
| Membidik secara manual | Seret jari menjauhi tombol Fire ke arah yang ingin ditembak |
| Mengisi ulang amunisi dan mengganti senjata | Tombol-tombol di atas Fire |
| Memilih senjata | Ketuk senjata pada bilah di bagian bawah |
| Menggunakan Shadowstep | Tombol Step (muncul saat kamu memegang suatu senjata) |
| Jeda dan papan skor | Tombol di pojok kiri atas (tombol Back juga menjeda permainan) |

Joystick bersifat mengambang (*floating joystick*). Letakkan ibu jari di bagian mana pun pada sisi kiri layar untuk mulai menggunakannya.

## Inspirasi dan kredit

Arcane Dome adalah game orisinal buatan penggemar yang **terinspirasi oleh seri game Flash _Raze_ karya JuiceTin dan AddisonR**. Terima kasih atas begitu banyak jam kesenangan yang telah diberikan oleh game-game tersebut.

Proyek ini **tidak berafiliasi dengan maupun didukung secara resmi oleh** para pembuat aslinya. Proyek ini tidak menggunakan karya seni, audio, kode, nama, ataupun tata letak peta dari game aslinya. Semua kode dan konten di sini merupakan karya orisinal atau digunakan berdasarkan lisensi terbuka yang tercantum dalam [CREDITS.md](CREDITS.md).

## Membangun dan menjalankan game

Memerlukan toolchain Rust versi stable.

```bash
cargo run --release
```

Untuk menjalankan pengujian:

```bash
cargo test
```

Di Linux, Macroquad membutuhkan pustaka sistem yang umum digunakan: ALSA, X11, Xi, serta driver OpenGL. Contohnya, pada Arch Linux, kamu bisa menggunakan paket berikut:

```bash
alsa-lib libx11 libxi mesa
```

### Android

Skrip `android/build-apk.sh` akan membangun APK di lokasi `target/android/arcane-dome-<version>.apk`.

Kamu memerlukan Android SDK melalui variabel lingkungan `ANDROID_HOME`, beserta NDK, build-tools, platform Android, JDK, dan target Rust berikut:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
```

Kemudian, jalankan:

```bash
ANDROID_HOME=~/Android/Sdk android/build-apk.sh
```

Jika tidak menggunakan kunci penandatanganan rilis, APK akan ditandatangani menggunakan kunci debug standar Android. Ini sudah cukup untuk mencoba game di ponselmu sendiri.

Variabel `ARCANE_TOUCH=1` menampilkan kontrol layar sentuh pada versi desktop sehingga kamu bisa memeriksa tata letaknya.

## Menambahkan konten

- **Terjemahan:** berkas `assets/lang/id.json` memetakan setiap teks bahasa Inggris ke versi Bahasa Indonesia. Penanda `{}` digunakan untuk mewakili angka atau nama. Jalankan `cargo test` untuk melihat teks yang terjemahannya masih belum tersedia.
- **Senjata:** satu berkas JSON untuk setiap senjata di `assets/data/weapons/`.
- **Peta:** satu berkas JSON untuk setiap peta di `assets/data/maps/`.
- **Power-up:** `assets/data/powerups.json`.

Tidak perlu mengubah kode untuk menambahkan senjata atau peta baru. Struktur data dan skemanya dijelaskan dalam [SPEC.md](SPEC.md).

### Peta

Peta dirancang secara manual menggunakan poligon bebas yang disebut `shapes`. Setiap bentuk memiliki jenis (`kind`):

- `solid`: menghalangi pergerakan karakter dan peluru dari semua arah.
- `hatch`: lantai tipis yang bisa dipijak dan ditembus saat turun. Pemain yang menggunakan thruster juga bisa menembusnya saat melesat ke atas dari bawah.
- `water`: memperlambat gerakan pemain dan memungkinkan mereka berenang.
- `hazard`: memberikan kerusakan kepada apa pun yang berada di dalamnya sesuai nilai `dps`. Bisa digunakan untuk lava, duri, atau jurang tanpa dasar.

Nilai `material` menentukan tampilan permukaan, misalnya batu, logam, kayu, pasir, lumut, lumpur, lava, dan sebagainya. Semua posisi dinyatakan dalam satuan ubin (*tiles*). Titik kemunculan, item, dan objek ditentukan berdasarkan posisi **kaki** (*feet*), yaitu titik tempat seorang petarung berdiri.

Objek yang tersedia meliputi thruster (`height_tiles`), stasiun senjata, stasiun perisai (`shield` dengan nilai 50 atau 100), dan titik kemunculan power-up (`powerup`, yang selalu menghasilkan jenis power-up yang sama).

Elemen dekorasi yang hanya bersifat visual disimpan di `decor`. Contohnya adalah dinding, jendela, balok konstruksi, rantai, lampu, pohon, dan sebagainya. Sementara itu, `theme` mengatur suasana peta melalui warna langit, matahari, dan garis cakrawala.

Titik kemunculan pemain dapat dimiliki oleh sebuah tim (indeks 0 hingga 3; pemain berada di tim 0) atau digunakan untuk mode *free-for-all*. Kepemilikan titik kemunculan bisa berubah sesuai jumlah tim melalui `by_teams`.

Setiap peta mencantumkan jumlah tim yang didukung dan dianggap seimbang melalui `team_counts`, misalnya `[2, 4]`.

Pastikan setiap titik kemunculan, item, dan objek bisa dijangkau oleh bot. Gunakan perintah berikut untuk memeriksanya:

```bash
cargo run --release --example map_check
```

Untuk memeriksa peta tertentu sekaligus menyimpan graf navigasinya dalam format JSON:

```bash
cargo run --release --example map_check -- foundry_ascent nav.json
```

Perintah kedua memeriksa satu peta dan menuliskan graf navigasinya ke berkas JSON.

Peta yang belum selesai disimpan di `drafts/maps/` dan tidak akan dimuat oleh game.

Untuk mengamati bot saat bermain di sebuah peta dan mendapatkan laporan jika ada bot yang tersangkut atau jatuh ke area berbahaya, gunakan:

```bash
cargo run --release --example bot_watch -- foundry_ascent 180 4
```

Tambahkan `MODE=elimination TEAMS=4` untuk pertandingan antartim atau `VERBOSE=1` untuk menampilkan informasi yang lebih terperinci.

## Peta jalan pengembangan (Roadmap)

Milestone 1 telah selesai (`v0.1.0`). Berikut daftar pekerjaan untuk Milestone 2:

1. Editor peta agar siapa pun bisa membuat dan berbagi peta (**selesai**).
2. Animasi dan tekstur.
3. Suara.
4. Kemungkinan penambahan mode Capture the Flag dan King of the Hill.

## Lisensi

Kode dirilis di bawah [Lisensi MIT](LICENSE). Aset pihak ketiga tetap mengikuti lisensinya masing-masing, yang tercantum dalam [CREDITS.md](CREDITS.md).
