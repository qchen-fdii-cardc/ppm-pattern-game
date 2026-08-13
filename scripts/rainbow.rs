let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let angle = (nx * std::f32::consts::PI * 2.0) + (ny * std::f32::consts::PI * 4.0);
let r = ((angle.sin() * 0.5 + 0.5) * 255.0) as u8;
let g = (((angle + std::f32::consts::PI * 2.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;
let b = (((angle + std::f32::consts::PI * 4.0 / 3.0).sin() * 0.5 + 0.5) * 255.0) as u8;