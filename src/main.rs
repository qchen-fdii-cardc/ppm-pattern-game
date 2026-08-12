use std::fs;

use iced::{
    Color, Length, Point, Size,
    widget::{button, canvas, column, row, slider, text, text_editor},
};
use rust_ppm::{Image, Pixel};

const DEFAULT_WIDTH: u32 = 128;
const DEFAULT_HEIGHT: u32 = 128;
const DEFAULT_PIXEL_FUN: &str = "gradient";
const SCRIPT_PATH: &str = "pixel_fun.txt";
const EXPORT_PATH: &str = "pattern.ppm";

struct PatternGame {
    width: u32,
    height: u32,
    image: Image,
    pixel_fun: String,
    script: text_editor::Content,
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
    ExportPPM,
    SavePixelFun,
    LoadPixelFun,
    PixelFunEdited(text_editor::Action),
}

impl PatternGame {
    fn new() -> Self {
        let mut game = Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            image: Image::new(1, 1),
            pixel_fun: DEFAULT_PIXEL_FUN.to_string(),
            script: text_editor::Content::with_text(DEFAULT_PIXEL_FUN),
        };
        game.refresh_pattern();
        game
    }

    fn refresh_pattern(&mut self) {
        self.pixel_fun = self.script.text();
        if self.pixel_fun.trim().is_empty() {
            self.pixel_fun = DEFAULT_PIXEL_FUN.to_string();
            self.script = text_editor::Content::with_text(&self.pixel_fun);
        }
        self.image = generate_pattern(
            self.width as usize,
            self.height as usize,
            self.pixel_fun.trim(),
        );
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let canvas = canvas(PpmCanvas {
            image: self.image.clone(),
        })
        .width(Length::Fill)
        .height(Length::Fixed(420.0));

        let editor = text_editor(&self.script)
            .width(220)
            .height(Length::Fixed(220.0))
            .padding(12)
            .on_action(Message::PixelFunEdited);

        column![
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
                button("Export PPM").on_press(Message::ExportPPM),
                button("Save Script").on_press(Message::SavePixelFun),
                button("Load Script").on_press(Message::LoadPixelFun),
            ]
            .spacing(10),
            row![
                column![text("Pattern Canvas"), canvas]
                    .spacing(8)
                    .width(Length::Fill),
                column![text("pixel_fun"), editor]
                    .spacing(8)
                    .width(Length::Fill),
            ]
            .spacing(12),
        ]
        .padding(20)
        .spacing(16)
        .into()
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
            Message::GeneratePattern => {
                self.refresh_pattern();
            }
            Message::ExportPPM => {
                let _ = self.image.to_file(EXPORT_PATH);
            }
            Message::SavePixelFun => {
                let _ = fs::write(SCRIPT_PATH, self.script.text());
            }
            Message::LoadPixelFun => {
                if let Ok(script) = fs::read_to_string(SCRIPT_PATH) {
                    self.script = text_editor::Content::with_text(&script);
                    self.pixel_fun = script;
                    self.refresh_pattern();
                }
            }
            Message::PixelFunEdited(action) => {
                self.script.perform(action);
                self.refresh_pattern();
            }
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

fn generate_pattern(width: usize, height: usize, pixel_fun: &str) -> Image {
    let pattern = pixel_fun.trim().to_ascii_lowercase();

    if pattern.contains("checker") {
        return checker_pattern(width, height);
    }

    if pattern.contains("wave") {
        return wave_pattern(width, height);
    }

    if pattern.contains("rainbow") || pattern.contains("hue") {
        return rainbow_pattern(width, height);
    }

    if pattern.contains("spiral") {
        return spiral_pattern(width, height);
    }

    gradient_pattern(width, height)
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
        let g = ((nx * 255.0) as f32) as u8;
        let b = ((ny * 255.0) as f32) as u8;
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
    let image = generate_pattern(32, 16, "gradient");
    assert_eq!(image.width, 32);
    assert_eq!(image.height, 16);
}

fn main() -> iced::Result {
    iced::run(PatternGame::update, PatternGame::view)
}
