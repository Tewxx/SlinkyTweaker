pub const UP: i32 = 1;
pub const HORIZONTALS: [i32; 4] = [3, 4, 2, 5];
pub const OFFSETS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

pub const LADDER: i32 = 65;
pub const VINE: i32 = 106;

const REPLACEABLE: u32 = 0x8000_0301;
const FULL_BLOCKS: &[i32] = &[
    1, 2, 3, 4, 5, 7, 12, 13, 14, 15, 16, 17, 19, 21, 22, 23, 24, 25, 35, 41, 42, 43, 45, 46, 47,
    48, 49, 56, 57, 58, 61, 62, 73, 74, 80, 82, 84, 86, 87, 88, 89, 91, 97, 98, 99, 100, 103, 110,
    112, 121, 123, 124, 125, 129, 133, 137, 152, 153, 155, 158, 159, 162, 168, 169, 170, 172, 173,
    174, 179, 181,
];
const NOT_SCAFFOLD: [i32; 11] = [12, 13, 23, 25, 46, 58, 61, 62, 86, 88, 137];

pub fn replaceable(id: i32) -> bool {
    (0..32).contains(&id) && REPLACEABLE & (1 << id) != 0
}
pub fn full(id: i32) -> bool {
    FULL_BLOCKS.contains(&id)
}
pub fn scaffold(id: i32) -> bool {
    full(id) && !NOT_SCAFFOLD.contains(&id)
}

pub fn offset(block: [i32; 3], side: i32) -> [i32; 3] {
    let o = OFFSETS[side as usize];
    [block[0] + o[0], block[1] + o[1], block[2] + o[2]]
}
pub fn normal(side: i32) -> [f64; 3] {
    OFFSETS[side as usize].map(|v| v as f64)
}
pub fn add(a: [f64; 3], b: [f64; 3], scale: f64) -> [f64; 3] {
    [a[0] + b[0] * scale, a[1] + b[1] * scale, a[2] + b[2] * scale]
}
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
pub fn centre(block: [i32; 3]) -> [f64; 3] {
    block.map(|v| v as f64 + 0.5)
}
pub fn face_centre(block: [i32; 3], side: i32) -> [f64; 3] {
    add(centre(block), normal(side), 0.5)
}
pub fn direction(yaw: f32, pitch: f32) -> [f64; 3] {
    let (yaw, pitch) = (
        (-yaw as f64).to_radians() - std::f64::consts::PI,
        (-pitch as f64).to_radians(),
    );
    [yaw.sin() * -pitch.cos(), pitch.sin(), yaw.cos() * -pitch.cos()]
}
pub fn quarter(yaw: f32) -> i32 {
    ((yaw * 4.0 / 360.0 + 0.5).floor() as i32).rem_euclid(4)
}
