//! 윈도우 트레이용 숫자 아이콘. 윈도우 트레이에는 글자를 붙일 수 없어서,
//! 가장 많이 찬 한도의 숫자를 아이콘 자체에 그린다(바탕은 서비스 색, 숫자는 어두운 색).
//! 글꼴 라이브러리 없이 5×7 점 숫자를 키워서 찍는다.

pub const SIZE: u32 = 64;
const INK: [u8; 3] = [0x1a, 0x12, 0x28];

/// 0~9의 5×7 점 그림. 한 줄이 5비트(왼쪽이 높은 자리).
const DIGITS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
];

/// "#rrggbb"를 읽는다. 모양이 다르면 None.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

/// 아이콘에 적을 숫자. 0~100으로 자르고 반올림한다(화면의 퍼센트 표기와 같게).
pub fn label(pct: f64) -> String {
    let v = if pct.is_finite() { pct.round().clamp(0.0, 100.0) } else { 0.0 };
    format!("{}", v as u32)
}

/// 64×64 RGBA 그림을 만든다(윈도우가 트레이 크기에 맞게 줄여 쓴다). 모서리가 둥근 색 바탕 위에 숫자를 가운데에 찍는다.
pub fn render(pct: f64, color: [u8; 3]) -> Vec<u8> {
    let n = SIZE as i32;
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];
    let radius = 12.0f32;
    for y in 0..n {
        for x in 0..n {
            // 모서리 네 곳만 둥글게: 모서리 원의 중심에서 반지름 밖이면 투명.
            let cx = (x as f32 + 0.5).clamp(radius, n as f32 - radius);
            let cy = (y as f32 + 0.5).clamp(radius, n as f32 - radius);
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy <= radius * radius {
                let i = ((y * n + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    let text = label(pct);
    let digits: Vec<usize> = text.bytes().map(|b| (b - b'0') as usize).collect();
    // 자릿수가 적을수록 크게 찍는다(점 하나의 픽셀 수). 좌우에 여백이 남게 고른 값이다.
    let (scale, gap): (i32, i32) = match digits.len() {
        1 => (6, 0),
        2 => (5, 4),
        _ => (3, 3),
    };
    let width = digits.len() as i32 * 5 * scale + (digits.len() as i32 - 1) * gap;
    let (x0, y0) = ((n - width) / 2, (n - 7 * scale) / 2);
    for (k, d) in digits.iter().enumerate() {
        let left = x0 + k as i32 * (5 * scale + gap);
        for (row, bits) in DIGITS[*d].iter().enumerate() {
            for col in 0..5 {
                if bits & (0b10000 >> col) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        let (x, y) = (left + col * scale + sx, y0 + row as i32 * scale + sy);
                        if (0..n).contains(&x) && (0..n).contains(&y) {
                            let i = ((y * n + x) * 4) as usize;
                            px[i..i + 4].copy_from_slice(&[INK[0], INK[1], INK[2], 255]);
                        }
                    }
                }
            }
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_bounds(px: &[u8]) -> (i32, i32, i32, i32, usize) {
        let (mut l, mut t, mut r, mut b, mut count) = (i32::MAX, i32::MAX, -1, -1, 0);
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let i = ((y * SIZE as i32 + x) * 4) as usize;
                if px[i..i + 3] == INK {
                    (l, t, r, b) = (l.min(x), t.min(y), r.max(x), b.max(y));
                    count += 1;
                }
            }
        }
        (l, t, r, b, count)
    }

    #[test]
    fn 색을_읽는다() {
        assert_eq!(parse_color("#ffa37d"), Some([0xff, 0xa3, 0x7d]));
        assert_eq!(parse_color("ffa37d"), None);
        assert_eq!(parse_color("#fff"), None);
        assert_eq!(parse_color("#gggggg"), None);
        assert_eq!(parse_color("#한글한"), None);
    }

    #[test]
    fn 숫자는_0에서_100_사이로_반올림한다() {
        assert_eq!(label(64.6), "65");
        assert_eq!(label(0.2), "0");
        assert_eq!(label(140.0), "100");
        assert_eq!(label(-3.0), "0");
        assert_eq!(label(f64::NAN), "0");
    }

    #[test]
    fn 그림_크기와_바탕색이_맞고_모서리는_투명하다() {
        let px = render(65.0, [0xff, 0xa3, 0x7d]);
        assert_eq!(px.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(px[3], 0, "왼쪽 위 모서리는 투명");
        let mid_top = ((SIZE + SIZE / 2) * 4) as usize;
        assert_eq!(px[mid_top..mid_top + 4], [0xff, 0xa3, 0x7d, 255]);
    }

    #[test]
    fn 숫자는_그림_안에_가운데로_들어간다() {
        for pct in [0.0, 7.0, 65.0, 99.0, 100.0] {
            let (l, t, r, b, count) = ink_bounds(&render(pct, [0x6f, 0xe3, 0xb8]));
            assert!(count > 20, "{pct}: 숫자가 그려져야 한다");
            let n = SIZE as i32;
            assert!(l >= 4 && t >= 4 && r <= n - 5 && b <= n - 5, "{pct}: 숫자가 가장자리에 닿았다 ({l},{t})~({r},{b})");
            assert!((l - (n - 1 - r)).abs() <= 2, "{pct}: 좌우 가운데가 아니다 ({l}, {r})");
        }
    }

    #[test]
    fn 숫자마다_그림이_다르다() {
        let a = render(64.0, [1, 2, 3]);
        let b = render(65.0, [1, 2, 3]);
        assert_ne!(a, b);
    }

    /// 점검용: DESKDUSK_BADGE_DUMP 폴더가 있으면 그림 원본을 내보낸다(눈으로 확인할 때).
    #[test]
    fn 점검용_그림_내보내기() {
        let Some(dir) = std::env::var_os("DESKDUSK_BADGE_DUMP") else { return };
        for (name, pct, color) in [("claude-65", 65.0, [0xff, 0xa3, 0x7d]), ("codex-7", 7.0, [0x6f, 0xe3, 0xb8]), ("claude-100", 100.0, [0xff, 0xa3, 0x7d])] {
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.rgba")), render(pct, color)).unwrap();
        }
    }
}
