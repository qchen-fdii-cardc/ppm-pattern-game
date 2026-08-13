use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use libloading::Library;
use rust_ppm::{Image, Pixel};

pub const DEFAULT_WIDTH: u32 = 128;
pub const DEFAULT_HEIGHT: u32 = 128;
pub const DEFAULT_SCRIPT_NAME: &str = "gradient.rs";
pub const SCRIPTS_DIR: &str = "scripts";
pub const EXPORT_PATH: &str = "pattern.ppm";
pub const TEMP_CACHE_DIR: &str = "ppm_pattern_game_cache";
pub const PREVIEW_WIDTH: f32 = 560.0;
pub const PREVIEW_MIN_HEIGHT: f32 = 180.0;
pub const PREVIEW_MAX_HEIGHT: f32 = 700.0;

pub fn ensure_script_directory(scripts_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(scripts_dir)?;

    let defaults = [
        ("gradient.rs", default_script_body("gradient.rs")),
        ("checker.rs", default_script_body("checker.rs")),
        ("wave.rs", default_script_body("wave.rs")),
        ("rainbow.rs", default_script_body("rainbow.rs")),
        ("spiral.rs", default_script_body("spiral.rs")),
    ];

    for (name, content) in defaults {
        let path = scripts_dir.join(name);
        if !path.exists() {
            fs::write(path, content)?;
        }
    }

    Ok(())
}

pub fn list_script_files(scripts_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(scripts_dir) else {
        return Vec::new();
    };

    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_rust = path.extension().and_then(|s| s.to_str()) == Some("rs");
            if is_rust {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    files.sort();
    files
}

pub fn default_script_body(script_name: &str) -> &'static str {
    match script_name {
        "checker.rs" => {
            r#"let cell = 12u32;
let on = ((x / cell) + (y / cell)) % 2 == 0;
let r = if on { 255 } else { 30 };
let g = if on { 255 } else { 30 };
let b = if on { 255 } else { 45 };"#
        }
        "wave.rs" => {
            r#"let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let wave = (nx * 18.0 + (ny * 18.0).sin() * 3.0).sin();
let r = ((wave + 1.0) * 127.5) as u8;
let g = (nx * 255.0) as u8;
let b = (ny * 255.0) as u8;"#
        }
        "rainbow.rs" => {
            r#"let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let angle = (nx * std::f32::consts::PI * 2.0) + (ny * std::f32::consts::PI * 4.0);
let r = ((angle.sin() * 0.5 + 0.5) * 255.0) as u8;
let g = (((angle + std::f32::consts::PI * 2.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;
let b = (((angle + std::f32::consts::PI * 4.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;"#
        }
        "spiral.rs" => {
            r#"let cx = width as f32 / 2.0;
let cy = height as f32 / 2.0;
let dx = x as f32 - cx;
let dy = y as f32 - cy;
let radius = (dx * dx + dy * dy).sqrt();
let angle = (dy / (dx + 0.0001)).atan();
let value = (radius * 0.18 + angle * 24.0).sin();
let r = ((value + 1.0) * 127.5) as u8;
let g = (radius as u8).wrapping_add(50);
let b = (angle as f32 * 30.0) as u8;"#
        }
        _ => {
            r#"let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let r = (nx * 255.0) as u8;
let g = (ny * 255.0) as u8;
let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);"#
        }
    }
}

pub struct PpmCanvas {
    pub image: Image,
}

impl<Message> iced::widget::canvas::Program<Message> for PpmCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        _renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let mut frame = iced::widget::canvas::Frame::new(_renderer, bounds.size());

        let width = self.image.width.max(1);
        let height = self.image.height.max(1);

        let cell_w = bounds.width / width as f32;
        let cell_h = bounds.height / height as f32;

        for y in 0..self.image.height {
            for x in 0..self.image.width {
                let Some(pixel) = self.image.get_pixel(x, y) else {
                    continue;
                };

                let rect = iced::widget::canvas::Path::rectangle(
                    iced::Point::new(x as f32 * cell_w, y as f32 * cell_h),
                    iced::Size::new(cell_w.max(1.0), cell_h.max(1.0)),
                );

                frame.fill(&rect, iced::Color::from_rgb8(pixel.r, pixel.g, pixel.b));
            }
        }

        vec![frame.into_geometry()]
    }
}

pub struct CompiledPattern {
    _library_path: PathBuf,
    library: Library,
}

impl CompiledPattern {
    pub fn new(script_path: &Path) -> Result<Self, String> {
        let temp_root = std::env::temp_dir().join(TEMP_CACHE_DIR);
        fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;

        let stem = script_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("pattern");
        let output_path = temp_root.join(format!("{stem}{}", lib_file_extension_with_dot()));
        let compile_source_path = temp_root.join(format!("{stem}.generated.rs"));

        let source_body = fs::read_to_string(script_path).map_err(|e| e.to_string())?;
        let generated_source = render_pattern_source(&source_body);
        fs::write(&compile_source_path, generated_source).map_err(|e| e.to_string())?;

        let script_modified = script_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        let library_modified = output_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        let should_rebuild = !output_path.exists() || script_modified > library_modified;

        if !should_rebuild {
            return Self::load_from_path(output_path);
        }

        let source_path_str = compile_source_path
            .to_str()
            .ok_or_else(|| "source path is not valid utf-8".to_owned())?;
        let output_path_str = output_path
            .to_str()
            .ok_or_else(|| "output path is not valid utf-8".to_owned())?;

        compile_rust_library(stem, source_path_str, output_path_str).map_err(
            |compiler_message| {
                format!(
                    "rustc compilation failed for script: {}\n{}",
                    script_path.display(),
                    compiler_message.trim()
                )
            },
        )?;

        Self::load_from_path(output_path)
    }

    fn load_from_path(path: PathBuf) -> Result<Self, String> {
        let library = unsafe {
            Library::new(&path).map_err(|e| format!("failed to load dynamic library: {e}"))?
        };

        Ok(Self {
            _library_path: path,
            library,
        })
    }

    pub fn render(&self, width: usize, height: usize) -> Image {
        let pixel_fn = unsafe {
            self.library
                .get::<unsafe extern "C" fn(u32, u32, u32, u32) -> u32>(b"pixel_at")
                .expect("pixel_at symbol missing")
        };

        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let packed = unsafe { pixel_fn(x as u32, y as u32, width as u32, height as u32) };
                let r = ((packed >> 16) & 0xff) as u8;
                let g = ((packed >> 8) & 0xff) as u8;
                let b = (packed & 0xff) as u8;
                pixels.push(Pixel::rgb(r, g, b));
            }
        }

        Image::from_pixels(width, height, pixels)
    }
}

pub fn render_pattern_source(body: &str) -> String {
    let normalized_body = body.trim();
    let body_expr = if normalized_body.contains("((r as u32) << 16)") {
        normalized_body.to_string()
    } else {
        format!(
            "{}\n((r as u32) << 16) | ((g as u32) << 8) | (b as u32)",
            normalized_body
        )
    };

    format!(
        r#"
        #[no_mangle]
        pub extern "C" fn pixel_at(x: u32, y: u32, width: u32, height: u32) -> u32 {{
            let _ = (x, y, width, height);
            {body_expr}
        }}
        "#
    )
}

pub fn compile_rust_library(
    crate_name: &str,
    source_path: &str,
    output_path: &str,
) -> Result<(), String> {
    let status = Command::new("rustc")
        .args([
            "--crate-type",
            "cdylib",
            "--crate-name",
            crate_name,
            source_path,
            "-o",
            output_path,
        ])
        .status()
        .map_err(|e| format!("failed to invoke rustc: {e}"))?;

    if status.success() {
        return Ok(());
    }

    let output = Command::new("rustc")
        .args([
            "--crate-type",
            "cdylib",
            "--crate-name",
            crate_name,
            source_path,
            "-o",
            output_path,
        ])
        .output()
        .map_err(|e| format!("failed to invoke rustc: {e}"))?;

    let compiler_message = String::from_utf8_lossy(&output.stderr);
    Err(compiler_message.trim().to_owned())
}

pub fn lib_file_extension_with_dot() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

pub fn generate_pattern(
    width: usize,
    height: usize,
    script_path: &Path,
    script_name: &str,
) -> (Image, Option<String>) {
    let pattern = script_name.trim().to_ascii_lowercase();

    match CompiledPattern::new(script_path) {
        Ok(compiled) => (compiled.render(width, height), None),
        Err(err) => {
            eprintln!(
                "dynamic library pattern generation failed: {err}. Falling back to built-in generator."
            );
            let fallback = match pattern.as_str() {
                s if s.contains("checker") => checker_pattern(width, height),
                s if s.contains("wave") => wave_pattern(width, height),
                s if s.contains("rainbow") || s.contains("hue") => rainbow_pattern(width, height),
                s if s.contains("spiral") => spiral_pattern(width, height),
                _ => gradient_pattern(width, height),
            };
            (fallback, Some(err))
        }
    }
}

pub fn gradient_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let nx = x as f32 / width.max(1) as f32;
        let ny = y as f32 / height.max(1) as f32;
        let r = (nx * 255.0) as u8;
        let g = (ny * 255.0) as u8;
        let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);
        Pixel::rgb(r, g, b)
    })
}

pub fn checker_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let cell = 12usize;
        let on = ((x / cell) + (y / cell)) % 2 == 0;
        if on {
            Pixel::rgb(255, 255, 255)
        } else {
            Pixel::rgb(30, 30, 45)
        }
    })
}

pub fn wave_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let nx = x as f32 / width.max(1) as f32;
        let ny = y as f32 / height.max(1) as f32;
        let wave = (nx * 18.0 + (ny * 18.0).sin() * 3.0).sin();
        let r = ((wave + 1.0) * 127.5) as u8;
        let g = (nx * 255.0) as u8;
        let b = (ny * 255.0) as u8;
        Pixel::rgb(r, g, b)
    })
}

pub fn rainbow_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let nx = x as f32 / width.max(1) as f32;
        let ny = y as f32 / height.max(1) as f32;
        let angle = (nx * std::f32::consts::PI * 2.0) + (ny * std::f32::consts::PI * 4.0);
        let r = ((angle.sin() * 0.5 + 0.5) * 255.0) as u8;
        let g = (((angle + std::f32::consts::PI * 2.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;
        let b = (((angle + std::f32::consts::PI * 4.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;
        Pixel::rgb(r, g, b)
    })
}

pub fn spiral_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        let radius = (dx * dx + dy * dy).sqrt();
        let angle = (dy / (dx + 0.0001)).atan();
        let value = (radius * 0.18 + angle * 24.0).sin();
        let r = ((value + 1.0) * 127.5) as u8;
        let g = (radius as u8).wrapping_add(50);
        let b = (angle as f32 * 30.0) as u8;
        Pixel::rgb(r, g, b)
    })
}
