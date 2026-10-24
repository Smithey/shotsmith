use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::RgbaImage;
use shotsmith_core::ImageFormat;
use shotsmith_ocr::{plain_text, recognize};
use shotsmith_output::encode;

fn main() {
    let mut args = std::env::args().skip(1);
    let code = match args.next().as_deref() {
        Some("capture") => capture(args.collect()),
        Some("ocr") => ocr(args.next()),
        Some("presets") => presets(),
        Some("watch") => watch(args.next()),
        _ => {
            help();
            2
        }
    };
    std::process::exit(code);
}

fn help() {
    println!(
        "shotsmith capture --screen --out file.png\nshotsmith ocr file.png\nshotsmith presets\nshotsmith watch folder"
    );
}

fn capture(args: Vec<String>) -> i32 {
    let mut screen = false;
    let mut out = PathBuf::from("capture.png");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--screen" => screen = true,
            "--out" => {
                index += 1;
                if let Some(path) = args.get(index) {
                    out = PathBuf::from(path);
                }
            }
            other => {
                eprintln!("Unknown option {other}.");
                return 2;
            }
        }
        index += 1;
    }
    if !screen {
        eprintln!("shotsmith capture currently takes a full screen. Pass --screen.");
        return 2;
    }
    match std::thread::spawn(shotsmith_platform::freeze_all).join() {
        Ok(Ok(mut displays)) => {
            if displays.is_empty() {
                eprintln!("No display was captured.");
                return 1;
            }
            let index = displays
                .iter()
                .position(|item| item.info.primary)
                .unwrap_or(0);
            let display = displays.swap_remove(index);
            write_png(&display.image, &out)
        }
        Ok(Err(err)) => {
            eprintln!("{err}");
            1
        }
        Err(_) => {
            eprintln!("Screen capture failed.");
            1
        }
    }
}

fn ocr(path: Option<String>) -> i32 {
    let Some(path) = path else {
        eprintln!("shotsmith ocr file.png");
        return 2;
    };
    match image::open(&path).map(|image| image.to_rgba8()) {
        Ok(image) => match recognize(&image) {
            Ok(lines) => {
                println!("{}", plain_text(&lines));
                0
            }
            Err(err) => {
                eprintln!("{err}");
                1
            }
        },
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn presets() -> i32 {
    let path = data_root().join("presets.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("No presets yet. Open Shotsmith once, then try again.");
        return 1;
    };
    let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&text) else {
        eprintln!("Could not read {}.", path.display());
        return 1;
    };
    for item in list {
        let name = item
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Preset");
        let id = item
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        println!("{name}\t{id}");
    }
    0
}

fn watch(folder: Option<String>) -> i32 {
    let Some(folder) = folder else {
        eprintln!("shotsmith watch folder");
        return 2;
    };
    let folder = PathBuf::from(folder);
    if let Err(err) = std::fs::create_dir_all(&folder) {
        eprintln!("{err}");
        return 1;
    }
    println!(
        "Watching {}. A file is reported once its size stays the same for a second.",
        folder.display()
    );
    let mut seen: Vec<(PathBuf, u64, Instant, bool)> = Vec::new();
    loop {
        let Ok(read) = std::fs::read_dir(&folder) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if !is_image(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let size = meta.len();
            if let Some(slot) = seen.iter_mut().find(|(item, _, _, _)| item == &path) {
                if slot.3 {
                    continue;
                }
                if slot.1 != size {
                    slot.1 = size;
                    slot.2 = Instant::now();
                } else if slot.2.elapsed() >= Duration::from_secs(1) {
                    println!("{}", path.display());
                    slot.3 = true;
                }
            } else {
                seen.push((path, size, Instant::now(), false));
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn write_png(image: &RgbaImage, path: &Path) -> i32 {
    match encode(image, ImageFormat::Png, 1.0) {
        Ok(bytes) => match std::fs::write(path, bytes) {
            Ok(()) => {
                println!("{}", path.display());
                0
            }
            Err(err) => {
                eprintln!("{err}");
                1
            }
        },
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff")
    )
}

fn data_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Shotsmith")
}
