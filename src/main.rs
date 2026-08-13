use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use iced::{
    Color, Length, Point, Size,
    widget::{
        button, canvas, column, container, pick_list, progress_bar, row, slider, text, text_editor,
    },
};
use libloading::Library;
use rust_ppm::{Image, Pixel};

const DEFAULT_WIDTH: u32 = 128;
const DEFAULT_HEIGHT: u32 = 128;
const DEFAULT_SCRIPT_NAME: &str = "gradient.rs";
const SCRIPTS_DIR: &str = "scripts";
const EXPORT_PATH: &str = "pattern.ppm";
const TEMP_CACHE_DIR: &str = "ppm_pattern_game_cache";
const PREVIEW_WIDTH: f32 = 560.0;
const PREVIEW_MIN_HEIGHT: f32 = 180.0;
const PREVIEW_MAX_HEIGHT: f32 = 700.0;

struct PatternGame {
    width: u32,
    height: u32,
    image: Image,
    pixel_fun: String,
    script: text_editor::Content,
    scripts_dir: PathBuf,
    script_files: Vec<String>,
    selected_script: String,
    compile_progress: f32,
    compile_status: String,
    compile_error: Option<String>,
}

impl Default for PatternGame {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    WidthChanged(u32),
    HeightChanged(u32),
    GeneratePattern,
    RenderScript,
    ExportPPM,
    NewScript,
    SaveScript,
    LoadScript,
    ScriptSelected(String),
    PixelFunEdited(text_editor::Action),
    ClearCompileError,
}

impl PatternGame {
    fn new() -> Self {
        let scripts_dir = PathBuf::from(SCRIPTS_DIR);
        let _ = ensure_script_directory(&scripts_dir);

        let mut script_files = list_script_files(&scripts_dir);
        if script_files.is_empty() {
            let _ = ensure_script_directory(&scripts_dir);
            script_files = list_script_files(&scripts_dir);
        }

        let selected_script = script_files
            .first()
            .cloned()
            .unwrap_or_else(|| DEFAULT_SCRIPT_NAME.to_string());

        let script_path = scripts_dir.join(&selected_script);
        let script_text = fs::read_to_string(&script_path)
            .unwrap_or_else(|_| default_script_body(&selected_script).to_string());

        let mut game = Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            image: Image::new(1, 1),
            pixel_fun: script_text.clone(),
            script: text_editor::Content::with_text(&script_text),
            scripts_dir,
            script_files,
            selected_script,
            compile_progress: 1.0,
            compile_status: "Ready".to_string(),
            compile_error: None,
        };
        game.refresh_pattern();
        game
    }

    fn script_path(&self) -> PathBuf {
        self.scripts_dir.join(&self.selected_script)
    }

    fn preview_dimensions(&self) -> (f32, f32) {
        let ratio = self.width.max(1) as f32 / self.height.max(1) as f32;
        let preview_height =
            (PREVIEW_WIDTH / ratio.max(0.0001)).clamp(PREVIEW_MIN_HEIGHT, PREVIEW_MAX_HEIGHT);
        (PREVIEW_WIDTH, preview_height)
    }

    fn refresh_script_list(&mut self) {
        let mut script_files = list_script_files(&self.scripts_dir);
        script_files.sort();
        self.script_files = script_files;

        if self.selected_script.is_empty()
            || !self
                .script_files
                .iter()
                .any(|name| name == &self.selected_script)
        {
            self.selected_script = self
                .script_files
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_SCRIPT_NAME.to_string());
        }
    }

    fn persist_script(&self) -> io::Result<()> {
        fs::create_dir_all(&self.scripts_dir)?;
        fs::write(self.script_path(), self.script.text())
    }

    fn refresh_pattern(&mut self) {
        self.pixel_fun = self.script.text();
        if self.pixel_fun.trim().is_empty() {
            self.pixel_fun = default_script_body(&self.selected_script).to_string();
            self.script = text_editor::Content::with_text(&self.pixel_fun);
        }

        self.compile_progress = 0.2;
        self.compile_status = "Saving script...".to_string();
        self.compile_error = None;

        if let Err(err) = self.persist_script() {
            self.compile_progress = 0.0;
            self.compile_status = format!("Save failed: {err}");
            self.compile_error = Some(err.to_string());
            return;
        }

        self.compile_progress = 0.7;
        self.compile_status = "Compiling library...".to_string();

        let (image, compile_error) = generate_pattern(
            self.width as usize,
            self.height as usize,
            &self.script_path(),
            self.selected_script.trim(),
        );
        self.image = image;

        self.compile_progress = 1.0;
        if let Some(err) = compile_error {
            self.compile_status = format!("Compile warning: using fallback ({err})");
            self.compile_error = Some(err);
        } else {
            self.compile_status = "Ready".to_string();
            self.compile_error = None;
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let (_, preview_height) = self.preview_dimensions();
        let preview_canvas = canvas(PpmCanvas {
            image: self.image.clone(),
        })
        .width(Length::Fill)
        .height(Length::Fixed(preview_height));

        let script_picker = pick_list(
            self.script_files.clone(),
            Some(self.selected_script.clone()),
            Message::ScriptSelected,
        )
        .placeholder("Select script")
        .width(Length::Fill);

        let script_tools = {
            let mut tools = column![
                text("Script"),
                script_picker,
                text(&self.compile_status).width(Length::Fill),
                container(progress_bar(0.0..=1.0, self.compile_progress)).width(Length::Fill),
            ]
            .spacing(8)
            .width(Length::Fill);

            if let Some(message) = &self.compile_error {
                tools = tools.push(
                    container(
                        column![
                            text("Compile Error").size(18),
                            text(message).color(Color::from_rgb8(255, 110, 110)),
                            button("Close").on_press(Message::ClearCompileError),
                        ]
                        .spacing(8)
                        .width(Length::Fill),
                    )
                    .padding([8, 10])
                    .width(Length::Fill),
                );
            }

            tools.push(
                container(
                    text_editor(&self.script)
                        .width(420)
                        .height(Length::Fixed(320.0))
                        .padding(12)
                        .on_action(Message::PixelFunEdited),
                )
                .width(Length::Fill)
                .height(Length::Fill),
            )
        };

        let preview_column = column![text("Pattern Canvas"), preview_canvas]
            .spacing(8)
            .width(Length::FillPortion(1));

        let script_column = container(script_tools).width(Length::FillPortion(1));

        let main_content = column![
            text("PPM Pattern Game").size(32),
            row![
                text("Width"),
                slider(16.0..=256.0, self.width as f32, |value| {
                    Message::WidthChanged(value as u32)
                })
                .step(1.0),
                text(self.width),
            ]
            .spacing(12),
            row![
                text("Height"),
                slider(16.0..=256.0, self.height as f32, |value| {
                    Message::HeightChanged(value as u32)
                })
                .step(1.0),
                text(self.height),
            ]
            .spacing(12),
            row![
                button("Generate Pattern").on_press(Message::GeneratePattern),
                button("Render Script").on_press(Message::RenderScript),
                button("New Script").on_press(Message::NewScript),
                button("Export PPM").on_press(Message::ExportPPM),
                button("Save Script").on_press(Message::SaveScript),
                button("Reload Script").on_press(Message::LoadScript),
            ]
            .spacing(10),
            row![preview_column, script_column]
                .spacing(12)
                .width(Length::Fill),
        ]
        .padding(20)
        .spacing(16)
        .width(Length::Fill);

        main_content.into()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::WidthChanged(value) => {
                self.width = value;
                self.refresh_pattern();
            }
            Message::HeightChanged(value) => {
                self.height = value;
                self.refresh_pattern();
            }
            Message::GeneratePattern | Message::RenderScript => {
                self.refresh_pattern();
            }
            Message::ExportPPM => {
                let _ = self.image.to_file(EXPORT_PATH);
            }
            Message::NewScript => {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let name = format!("custom_{timestamp}.rs");
                let path = self.scripts_dir.join(&name);
                let content = default_script_body("custom.rs");
                if let Err(err) = fs::write(&path, content) {
                    self.compile_error = Some(format!("Create failed: {err}"));
                    self.compile_status = format!("Create failed: {err}");
                    return;
                }

                self.selected_script = name;
                self.script = text_editor::Content::with_text(content);
                self.pixel_fun = content.to_string();
                self.refresh_script_list();
                self.refresh_pattern();
            }
            Message::SaveScript => {
                if let Err(err) = self.persist_script() {
                    self.compile_error = Some(format!("Save failed: {err}"));
                    self.compile_status = format!("Save failed: {err}");
                    return;
                }
                self.refresh_script_list();
                self.compile_status = "Saved".to_string();
                self.compile_error = None;
            }
            Message::LoadScript => {
                if let Ok(script) = fs::read_to_string(self.script_path()) {
                    self.script = text_editor::Content::with_text(&script);
                    self.pixel_fun = script;
                    self.refresh_pattern();
                }
            }
            Message::ScriptSelected(name) => {
                self.selected_script = name.clone();
                if let Ok(script) = fs::read_to_string(self.script_path()) {
                    self.script = text_editor::Content::with_text(&script);
                    self.pixel_fun = script;
                    self.refresh_pattern();
                }
            }
            Message::PixelFunEdited(action) => {
                self.script.perform(action);
                self.compile_status = "Script edited; click Render Script to compile".to_string();
                self.compile_error = None;
            }
            Message::ClearCompileError => {
                self.compile_error = None;
                self.compile_status = "Ready".to_string();
            }
        }
    }
}

fn ensure_script_directory(scripts_dir: &Path) -> io::Result<()> {
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

fn list_script_files(scripts_dir: &Path) -> Vec<String> {
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

fn default_script_body(script_name: &str) -> &'static str {
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

struct PpmCanvas {
    image: Image,
}

impl<Message> canvas::Program<Message> for PpmCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        _renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(_renderer, bounds.size());

        let width = self.image.width.max(1);
        let height = self.image.height.max(1);

        let cell_w = bounds.width / width as f32;
        let cell_h = bounds.height / height as f32;

        for y in 0..self.image.height {
            for x in 0..self.image.width {
                let Some(pixel) = self.image.get_pixel(x, y) else {
                    continue;
                };

                let rect = canvas::Path::rectangle(
                    Point::new(x as f32 * cell_w, y as f32 * cell_h),
                    Size::new(cell_w.max(1.0), cell_h.max(1.0)),
                );

                frame.fill(&rect, Color::from_rgb8(pixel.r, pixel.g, pixel.b));
            }
        }

        vec![frame.into_geometry()]
    }
}

struct CompiledPattern {
    _library_path: PathBuf,
    library: Library,
}

impl CompiledPattern {
    fn new(script_path: &Path) -> Result<Self, String> {
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

    fn render(&self, width: usize, height: usize) -> Image {
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

fn render_pattern_source(body: &str) -> String {
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

fn compile_rust_library(
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

fn lib_file_extension_with_dot() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

fn generate_pattern(
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

fn gradient_pattern(width: usize, height: usize) -> Image {
    Image::from_pixel_fn(width, height, |x, y| {
        let nx = x as f32 / width.max(1) as f32;
        let ny = y as f32 / height.max(1) as f32;
        let r = (nx * 255.0) as u8;
        let g = (ny * 255.0) as u8;
        let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);
        Pixel::rgb(r, g, b)
    })
}

fn checker_pattern(width: usize, height: usize) -> Image {
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

fn wave_pattern(width: usize, height: usize) -> Image {
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

fn rainbow_pattern(width: usize, height: usize) -> Image {
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

fn spiral_pattern(width: usize, height: usize) -> Image {
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

#[test]
fn generated_pattern_has_requested_dimensions() {
    let script_path = PathBuf::from(SCRIPTS_DIR).join(DEFAULT_SCRIPT_NAME);
    let _ = ensure_script_directory(Path::new(SCRIPTS_DIR));
    let (image, _) = generate_pattern(32, 16, &script_path, DEFAULT_SCRIPT_NAME);
    assert_eq!(image.width, 32);
    assert_eq!(image.height, 16);
}

fn main() -> iced::Result {
    let _ = ensure_script_directory(Path::new(SCRIPTS_DIR));
    iced::run(PatternGame::update, PatternGame::view)
}
