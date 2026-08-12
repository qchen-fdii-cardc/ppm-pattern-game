# A simple ppm-based pattern rendering game 

Using rust-ppm to generate ppm images and render them in a simple game. The game is built using the iced GUI library.

1. A macro to generate ppm images from a pattern function, based on the `rust-ppm` crate.
2. A simple UI to display the generated ppm images.
3. Width and height of the ppm image can be adjusted using sliders and/or input fields.
4. The pattern function can be changed to generate different patterns.
5. ppm can be saved to a file using the export button.
6. pattern script can be saved and loaded using the save and load buttons.
7. a editor to edit the pattern script is provided.
8. hightlight the pattern script using the `syntect` crate or `highlight` crate or any other crate that can highlight code in rust.
9. format the pattern script using the `rustfmt` crate or any other crate that can format code in rust.

## Possible ways to edit the real pixel function on the fly

In a normal Rust binary, you cannot simply take a compiled function like this and mutate its body at runtime:

```rust
fn pixel_fn(x: usize, y: usize, width: usize, height: usize) -> Pixel {
    let nx = x as f32 / width.max(1) as f32;
    let ny = y as f32 / height.max(1) as f32;
    let r = (nx * 255.0) as u8;
    let g = (ny * 255.0) as u8;
    let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);
    Pixel::rgb(r, g, b)
}
```

Once the program has been compiled, Rust does not support hot-patching arbitrary machine code in the same way a dynamic language does. For a GUI app like this, the usual approaches are:

### 1. Interpret a small DSL instead of editing raw Rust code

This is the simplest and safest option for a game/editor app.

- The user edits text such as `r = x * 255 / w; g = y * 255 / h; b = (x + y) * 255 / (w + h);`
- The app parses that into a mini-language or expression AST.
- A runtime evaluator computes each pixel using the parsed expression.

This can be done with crates such as:

- `rhai` for an embedded scripting language
- `mlua` for Lua scripts
- `evalexpr` for simple expression evaluation
- a custom tiny parser for a restricted pattern language

A typical app design looks like this:

```rust
pub type PixelFn = fn(usize, usize, usize, usize) -> Pixel;

fn render_pixel(pixel_fn: PixelFn, x: usize, y: usize, width: usize, height: usize) -> Pixel {
    pixel_fn(x, y, width, height)
}
```

Pros:
- easy to edit live in a text box
- no recompilation required
- safer than arbitrary Rust execution

Cons:
- not full Rust syntax
- harder to support advanced logic

### 2. Generate Rust code from a text editor and compile it dynamically

This is the closest to “edit the real pixel function” while still staying in Rust.

- The text editor stores a string that represents a Rust function body, for example:

```rust
let nx = x as f32 / width.max(1) as f32;
let ny = y as f32 / height.max(1) as f32;
let r = (nx * 255.0) as u8;
let g = (ny * 255.0) as u8;
let b = (((nx + ny) * 127.5) as u8).wrapping_mul(2);
Pixel::rgb(r, g, b)
```

- The app wraps this into a full Rust function like:

```rust
fn generated_pixel_fn(x: usize, y: usize, width: usize, height: usize) -> Pixel {
    // user text inserted here
}
```

- Then the app writes a temporary crate or shared library and compiles it with `rustc` or `cargo`.
- Finally, it loads the compiled function using `libloading` or a similar runtime loader.

Useful crates:

- `syn` + `quote` for code generation
- `libloading` for loading a compiled `.so`/`.dylib`
- `tempfile` for temporary source files
- `rustc` or `cargo` subprocess calls

Pros:
- true Rust code editing
- can use exact Rust syntax and math
- best match for the project goal

Cons:
- more complex and slower
- requires compiling code at runtime
- safety and sandboxing are harder
- runtime crashes can be much more serious

### 3. Store a Rust-like function body, then compile only on Apply

This is the most practical compromise for a desktop app:

- the editor is a text area containing a Rust-like function body
- the user presses `Apply`
- the app wraps the string into a temporary source file
- the app compiles the function and swaps the current generator
- the new function is used immediately

This pattern is often called “compile-on-apply” or “hot reload style”, but it is not true in-process hot patching. It is usually a controlled reload.

### 4. Keep the real function fixed and edit parameters instead of code

If the project only needs a user-friendly editor, a simpler approach is to expose parameters instead of raw Rust source.

Examples:

- `red_scale`, `green_scale`, `blue_scale`
- `frequency`, `phase`, `mix`, `seed`
- `checker_size`, `warp`, `palette`

This avoids arbitrary code execution entirely and keeps the app stable.

Pros:
- very safe
- easy to validate
- good for “pattern game” UX

Cons:
- not as flexible as real code editing
- users cannot express arbitrary logic

## Recommended direction for this project

For this repo, the best balance is:

1. keep a text editor for a `pixel_fun` body
2. support a restricted Rust-like syntax
3. on `Apply`, generate a small Rust function and compile it in a temporary crate
4. if full Rust editing is too risky, fall back to a mini DSL or parameterized presets

That gives the best combination of:

- editable pattern logic
- actual Rust-like expressions
- a clear path to real PPM generation
- safe, maintainable app design

## Practical implementation idea

The project can define a common function signature such as:

```rust
pub type PixelGenerator = fn(usize, usize, usize, usize) -> Pixel;

fn evaluate_pixel(generator: PixelGenerator, x: usize, y: usize, width: usize, height: usize) -> Pixel {
    generator(x, y, width, height)
}
```

Then the app can switch between:

- a built-in generator
- an interpreted DSL generator
- a compiled temporary Rust generator

This keeps the rendering pipeline the same while making the pattern logic replaceable.
