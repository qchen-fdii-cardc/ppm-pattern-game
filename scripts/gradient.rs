let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let r = (nx * 255.0) as u8;
let g = (ny * 255.0) as u8;
let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);