//! Kare üretimi (saf, testli): NV12 ölçekleme ve "Telefon bekleniyor" yer tutucu kare.
//! Yer tutucu: koyu zemin (#0a0a0b) üzerinde altın (#e3a92b) 5x7 bitmap yazı, bir kez pişirilir.

/// RGB -> BT.709 sınırlı aralık YUV.
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let y = 16.0 + 0.1826 * r + 0.6142 * g + 0.0620 * b;
    let u = 128.0 - 0.1006 * r - 0.3386 * g + 0.4392 * b;
    let v = 128.0 + 0.4392 * r - 0.3989 * g - 0.0403 * b;
    (y.round().clamp(16.0, 235.0) as u8, u.round().clamp(16.0, 240.0) as u8, v.round().clamp(16.0, 240.0) as u8)
}

/// En yakın komşu NV12 ölçekleme (boyutlar >= 2 ve çift sayı olmalı). Aynı boyutsa kopya.
pub fn scale_nv12(src: &[u8], sw: usize, sh: usize, dst: &mut [u8], dw: usize, dh: usize) {
    if sw < 2
        || sh < 2
        || dw < 2
        || dh < 2
        || !sw.is_multiple_of(2)
        || !sh.is_multiple_of(2)
        || !dw.is_multiple_of(2)
        || !dh.is_multiple_of(2)
        || src.len() != sw * sh * 3 / 2
        || dst.len() != dw * dh * 3 / 2
    {
        return;
    }
    if sw == dw && sh == dh {
        dst.copy_from_slice(src);
        return;
    }
    for y in 0..dh {
        let sy = y * sh / dh;
        for x in 0..dw {
            dst[y * dw + x] = src[sy * sw + x * sw / dw];
        }
    }
    let (suv, duv) = (&src[sw * sh..], &mut dst[dw * dh..]);
    for y in 0..dh / 2 {
        let sy = y * (sh / 2) / (dh / 2);
        for x in 0..dw / 2 {
            let sx = x * (sw / 2) / (dw / 2);
            let (si, di) = (sy * sw + sx * 2, y * dw + x * 2);
            duv[di] = suv[si];
            duv[di + 1] = suv[si + 1];
        }
    }
}

// 5x7 yazı tipi: yalnızca "T-Bridge · Telefon bekleniyor" için gereken karakterler. Her satır 5 bit (MSB solda).
fn glyph(c: char) -> [u8; 7] {
    match c {
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'r' => [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        'i' => [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'd' => [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
        'g' => [0, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'e' => [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        '·' => [0, 0, 0, 0b00100, 0, 0, 0],
        'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        'o' => [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'n' => [0, 0, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'b' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110],
        'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
        'y' => [0, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        _ => [0; 7], // boşluk ve bilinmeyenler
    }
}

pub const PLACEHOLDER_TEXT: &str = "T-Bridge · Telefon bekleniyor";

/// Yer tutucu NV12 kare: #0a0a0b zemin, ortada altın yazı.
pub fn placeholder_nv12(w: usize, h: usize) -> Vec<u8> {
    let (bg_y, bg_u, bg_v) = rgb_to_yuv(0x0a, 0x0a, 0x0b);
    let (fy, fu, fv) = rgb_to_yuv(0xe3, 0xa9, 0x2b);
    let mut f = vec![0u8; w * h * 3 / 2];
    f[..w * h].fill(bg_y);
    for [u, v] in f[w * h..].as_chunks_mut::<2>().0 {
        *u = bg_u;
        *v = bg_v;
    }
    let chars: Vec<char> = PLACEHOLDER_TEXT.chars().collect();
    let scale = (w / (chars.len() * 6 + 2)).clamp(2, 12);
    let text_w = chars.len() * 6 * scale;
    let (x0, y0) = (((w.saturating_sub(text_w)) / 2) & !1, ((h.saturating_sub(7 * scale)) / 2) & !1);
    for (ci, ch) in chars.iter().enumerate() {
        let g = glyph(*ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = x0 + (ci * 6 + col) * scale + dx;
                        let y = y0 + row * scale + dy;
                        if x < w && y < h {
                            f[y * w + x] = fy;
                            let uv = w * h + (y / 2) * w + (x / 2) * 2;
                            f[uv] = fu;
                            f[uv + 1] = fv;
                        }
                    }
                }
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_background_and_gold_text() {
        let f = placeholder_nv12(1280, 720);
        assert_eq!(f.len(), 1280 * 720 * 3 / 2);
        let (by, _, _) = rgb_to_yuv(0x0a, 0x0a, 0x0b);
        assert_eq!(f[0], by, "sol üst köşe koyu zemin");
        let (gy, _, _) = rgb_to_yuv(0xe3, 0xa9, 0x2b);
        assert!(f[..1280 * 720].iter().any(|y| *y == gy), "altın yazı pikselleri var");
        assert!(by < gy);
    }

    #[test]
    fn scale_identity_and_down_and_up() {
        let src: Vec<u8> = (0..(8 * 4 * 3 / 2)).map(|i| i as u8).collect();
        let mut same = vec![0u8; src.len()];
        scale_nv12(&src, 8, 4, &mut same, 8, 4);
        assert_eq!(same, src);
        let mut half = vec![0u8; 4 * 2 * 3 / 2];
        scale_nv12(&src, 8, 4, &mut half, 4, 2);
        assert_eq!(half[0], src[0]);
        let mut up = vec![0u8; 16 * 8 * 3 / 2];
        scale_nv12(&src, 8, 4, &mut up, 16, 8);
        assert_eq!(up[0], src[0]);
        assert_eq!(up[15], src[7]);
    }

    #[test]
    fn yuv_limits() {
        assert_eq!(rgb_to_yuv(0, 0, 0).0, 16);
        assert_eq!(rgb_to_yuv(255, 255, 255).0, 235);
    }
}
