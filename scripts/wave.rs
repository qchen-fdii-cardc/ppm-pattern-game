let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let wave = (nx * 18.0 + (ny * 18.0).sin() * 3.0).sin();
let r = ((wave + 1.0) * 127.5) as u8;
let g = (nx * 255.0) as u8;
let b = (ny * 255.0) as u8;