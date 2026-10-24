//! 3x5 bitmap font. Each glyph is five octal digits, one per row; 4/2/1 are left/middle/right.

pub const ADVANCE: f32 = 4.0;

pub fn glyph(ch: char) -> u16 {
    match ch.to_ascii_uppercase() {
        'A' => 0o25755,
        'B' => 0o65656,
        'C' => 0o34443,
        'D' => 0o65556,
        'E' => 0o74647,
        'F' => 0o74644,
        'G' => 0o34553,
        'H' => 0o55755,
        'I' => 0o72227,
        'J' => 0o11152,
        'K' => 0o55655,
        'L' => 0o44447,
        'M' => 0o57755,
        'N' => 0o65555,
        'O' => 0o25552,
        'P' => 0o65644,
        'Q' => 0o25563,
        'R' => 0o65655,
        'S' => 0o34216,
        'T' => 0o72222,
        'U' => 0o55557,
        'V' => 0o55552,
        'W' => 0o55775,
        'X' => 0o55255,
        'Y' => 0o55222,
        'Z' => 0o71247,
        '0' => 0o75557,
        '1' => 0o26227,
        '2' => 0o61247,
        '3' => 0o61216,
        '4' => 0o55711,
        '5' => 0o74616,
        '6' => 0o34757,
        '7' => 0o71222,
        '8' => 0o75757,
        '9' => 0o75716,
        '!' => 0o22202,
        '?' => 0o61202,
        ':' => 0o02020,
        '-' => 0o00700,
        '+' => 0o02720,
        '←' => 0o24742,
        '→' => 0o21712,
        '↑' => 0o27222,
        '↓' => 0o22272,
        '.' => 0o00002,
        _ => 0,
    }
}

pub fn lit(glyph: u16, row: u32, col: u32) -> bool {
    let bits = (glyph >> (3 * (4 - row))) & 0o7;
    bits & (0o4 >> col) != 0
}

pub fn width(s: &str) -> f32 {
    let n = s.chars().count() as f32;
    if n == 0.0 { 0.0 } else { n * ADVANCE - 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rows_left_to_right() {
        let t = glyph('T');
        assert!((0..3).all(|c| lit(t, 0, c)));
        assert!(!lit(t, 1, 0) && lit(t, 1, 1) && !lit(t, 1, 2));
    }
}
