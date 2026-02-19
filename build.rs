// Uncomment these if you want to generate sample sprites
//
// use image::{Rgba, RgbaImage};
// use std::path::Path;
//
// fn draw_bordered_rect(width: u32, height: u32, fill: [u8; 4], border: [u8; 4]) -> RgbaImage {
//     let mut img = RgbaImage::new(width, height);
//     for y in 0..height {
//         for x in 0..width {
//             let on_border = x == 0 || x == width - 1 || y == 0 || y == height - 1;
//             img.put_pixel(x, y, Rgba(if on_border { border } else { fill }));
//         }
//     }
//     img
// }
//
// fn draw_wall(width: u32, height: u32) -> RgbaImage {
//     let mut img = RgbaImage::new(width, height);
//     for y in 0..height {
//         for x in 0..width {
//             // Brick-row offset: every other row of bricks shifts by half-width.
//             let brick_row = y / 6;
//             let offset_x = if brick_row % 2 == 0 { 0 } else { width / 2 };
//             let is_mortar_h = y % 6 == 0;
//             let is_mortar_v = (x + offset_x) % (width / 2) == 0;
//             let color = if is_mortar_h || is_mortar_v {
//                 Rgba([0x4A, 0x48, 0x45, 0xFF]) // dark mortar
//             } else {
//                 Rgba([0x82, 0x74, 0x66, 0xFF]) // warm stone
//             };
//             img.put_pixel(x, y, color);
//         }
//     }
//     img
// }
//
// fn draw_player(width: u32, height: u32) -> RgbaImage {
//     let mut img = RgbaImage::new(width, height);
//     // Yellow body
//     let body = Rgba([0xF5, 0xD0, 0x30, 0xFF]);
//     let dark = Rgba([0x80, 0x60, 0x00, 0xFF]);
//     let eye = Rgba([0x10, 0x10, 0x10, 0xFF]);
//
//     for y in 0..height {
//         for x in 0..width {
//             img.put_pixel(x, y, body);
//         }
//     }
//     // Thin dark border
//     for x in 0..width { img.put_pixel(x, 0, dark); img.put_pixel(x, height - 1, dark); }
//     for y in 0..height { img.put_pixel(0, y, dark); img.put_pixel(width - 1, y, dark); }
//     // Two pixel eyes at ~1/3 height
//     let ey = height / 3;
//     let ex1 = width / 4;
//     let ex2 = 3 * width / 4;
//     if ex1 < width && ey < height { img.put_pixel(ex1, ey, eye); }
//     if ex2 < width && ey < height { img.put_pixel(ex2, ey, eye); }
//     img
// }
//
// fn draw_small_enemy(width: u32, height: u32) -> RgbaImage {
//     let fill = [0xCC, 0x22, 0x22, 0xFF];
//     let border = [0x55, 0x00, 0x00, 0xFF];
//     let mut img = draw_bordered_rect(width, height, fill, border);
//     // Menacing eyes
//     let ey = height / 3;
//     let ex1 = width / 4;
//     let ex2 = 3 * width / 4;
//     let eye = Rgba([0xFF, 0xAA, 0x00, 0xFF]);
//     if ex1 < width && ey < height { img.put_pixel(ex1, ey, eye); }
//     if ex2 < width && ey < height { img.put_pixel(ex2, ey, eye); }
//     img
// }
//
// fn draw_big_enemy(width: u32, height: u32) -> RgbaImage {
//     let mut img = RgbaImage::new(width, height);
//     let body = Rgba([0x66, 0x00, 0x88, 0xFF]);
//     let border = Rgba([0xFF, 0x44, 0xFF, 0xFF]);
//     let eye = Rgba([0xFF, 0xFF, 0x00, 0xFF]);
//
//     for y in 0..height {
//         for x in 0..width {
//             let on_border = x == 0 || x == width - 1 || y == 0 || y == height - 1
//                 || x == 1 || x == width - 2 || y == 1 || y == height - 2;
//             img.put_pixel(x, y, if on_border { border } else { body });
//         }
//     }
//     // Large eyes
//     let ey = height / 3;
//     let ex1 = width / 4;
//     let ex2 = 3 * width / 4;
//     for dy in 0..3u32 {
//         for dx in 0..3u32 {
//             if ex1 + dx < width && ey + dy < height { img.put_pixel(ex1 + dx - 1, ey + dy - 1, eye); }
//             if ex2 + dx < width && ey + dy < height { img.put_pixel(ex2 + dx - 1, ey + dy - 1, eye); }
//         }
//     }
//     img
// }
//
// fn save_if_missing(path: &str, img: RgbaImage) {
//     if !Path::new(path).exists() {
//         img.save(path).unwrap_or_else(|e| eprintln!("build: could not save {path}: {e}"));
//     }
// }
//
// fn main() {
//     let dir = "resources/sprites";
//     std::fs::create_dir_all(dir).expect("build: failed to create resources/sprites/");
//
//     save_if_missing(&format!("{dir}/player.png"), draw_player(16, 24));
//     save_if_missing(&format!("{dir}/small_enemy.png"), draw_small_enemy(16, 24));
//     save_if_missing(&format!("{dir}/big_enemy.png"), draw_big_enemy(32, 48));
//     save_if_missing(&format!("{dir}/wall.png"), draw_wall(16, 24));
//
//     println!("cargo:rerun-if-changed=build.rs");
// }
fn main() {

}