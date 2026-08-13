use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use iced::{
    Color, Length,
    widget::{
        button, canvas, column, container, pick_list, progress_bar, row, slider, text, text_editor,
    },
};
use rust_ppm::Image;

use crate::patterns::{
    DEFAULT_HEIGHT, DEFAULT_SCRIPT_NAME, DEFAULT_WIDTH, EXPORT_PATH, PREVIEW_MAX_HEIGHT,
    PREVIEW_MIN_HEIGHT, PREVIEW_WIDTH, PpmCanvas, SCRIPTS_DIR, default_script_body,
    ensure_script_directory, generate_pattern, list_script_files,
};

pub struct PatternGame {
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
    pub fn new() -> Self {
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

    pub fn view(&self) -> iced::Element<'_, Message> {
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

    pub fn update(&mut self, message: Message) {
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

pub fn run() -> iced::Result {
    let _ = ensure_script_directory(Path::new(SCRIPTS_DIR));
    iced::run(PatternGame::update, PatternGame::view)
}
