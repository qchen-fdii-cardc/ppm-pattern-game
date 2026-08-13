let cell = 12u32;
let on = ((x / cell) + (y / cell)) % 2 == 0;
let r = if on { 255 } else { 30 };
let g = if on { 255 } else { 30 };
let b = if on { 255 } else { 45 };