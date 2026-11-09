//! Tuş tutma (`key-hold`): telefondan PC'ye KeyDown/KeyUp. Platformdan bağımsız tuş kodları, Windows sanal tuş + tarama
//! kodu eşlemesi (SendInput için) ve basılı tutulan tuşların izlenmesi (bağlantı kopunca/ekrandan çıkınca bırakmak için).
use serde::{Deserialize, Serialize};

/// Tutulabilen/basılabilen tuş. Yeni tuş = SONA eklenir (postcard varyant dizini sabit kalsın).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum KeyCode {
    Ctrl,
    Alt,
    Shift,
    Win,
    Esc,
    Tab,
    Enter,
    Backspace,
    Delete,
    Insert,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    A,
    C,
    D,
    E,
    L,
    R,
    S,
    V,
    X,
    Y,
    Z,
}

/// SendInput için tuş tanımı: sanal tuş kodu + tarama kodu (set 1) + genişletilmiş tuş bayrağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    pub vk: u16,
    pub scan: u16,
    pub extended: bool,
}

impl KeyCode {
    /// Değiştirici tuş mu (Ctrl/Alt/Shift/Win).
    pub fn is_modifier(self) -> bool {
        matches!(self, KeyCode::Ctrl | KeyCode::Alt | KeyCode::Shift | KeyCode::Win)
    }

    /// Windows sanal tuş kodu, tarama kodu ve genişletilmiş bayrağı. Sol değiştiriciler kullanılır (VK_L*).
    pub fn spec(self) -> KeySpec {
        use KeyCode::*;
        let (vk, scan, extended) = match self {
            Ctrl => (0xA2, 0x1D, false),
            Alt => (0xA4, 0x38, false),
            Shift => (0xA0, 0x2A, false),
            Win => (0x5B, 0x5B, true),
            Esc => (0x1B, 0x01, false),
            Tab => (0x09, 0x0F, false),
            Enter => (0x0D, 0x1C, false),
            Backspace => (0x08, 0x0E, false),
            Delete => (0x2E, 0x53, true),
            Insert => (0x2D, 0x52, true),
            Up => (0x26, 0x48, true),
            Down => (0x28, 0x50, true),
            Left => (0x25, 0x4B, true),
            Right => (0x27, 0x4D, true),
            Home => (0x24, 0x47, true),
            End => (0x23, 0x4F, true),
            PageUp => (0x21, 0x49, true),
            PageDown => (0x22, 0x51, true),
            F1 => (0x70, 0x3B, false),
            F2 => (0x71, 0x3C, false),
            F3 => (0x72, 0x3D, false),
            F4 => (0x73, 0x3E, false),
            F5 => (0x74, 0x3F, false),
            F6 => (0x75, 0x40, false),
            F7 => (0x76, 0x41, false),
            F8 => (0x77, 0x42, false),
            F9 => (0x78, 0x43, false),
            F10 => (0x79, 0x44, false),
            F11 => (0x7A, 0x57, false),
            F12 => (0x7B, 0x58, false),
            A => (0x41, 0x1E, false),
            C => (0x43, 0x2E, false),
            D => (0x44, 0x20, false),
            E => (0x45, 0x12, false),
            L => (0x4C, 0x26, false),
            R => (0x52, 0x13, false),
            S => (0x53, 0x1F, false),
            V => (0x56, 0x2F, false),
            X => (0x58, 0x2D, false),
            Y => (0x59, 0x15, false),
            Z => (0x5A, 0x2C, false),
        };
        KeySpec { vk, scan, extended }
    }
}

/// Basılı tutulan tuşların kaydı (PC tarafı). Bağlantı kopunca/ekrandan çıkınca `release_all` ile hepsi bırakılır;
/// böylece Ctrl/Alt/Shift/Win PC'de takılı kalmaz.
#[derive(Debug, Default)]
pub struct HeldKeys {
    order: Vec<KeyCode>,
}

impl HeldKeys {
    pub const fn new() -> Self {
        Self { order: Vec::new() }
    }

    /// Tuş basıldı. Zaten basılıysa yeniden eklenmez (çift KeyDown tek bırakma ile kapanır).
    pub fn down(&mut self, k: KeyCode) {
        if !self.order.contains(&k) {
            self.order.push(k);
        }
    }

    /// Tuş bırakıldı.
    pub fn up(&mut self, k: KeyCode) {
        self.order.retain(|x| *x != k);
    }

    pub fn is_held(&self, k: KeyCode) -> bool {
        self.order.contains(&k)
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Basılı tüm tuşları döndürür (basılış sırasının TERSİ: sonra basılan önce bırakılır) ve kaydı temizler.
    pub fn release_all(&mut self) -> Vec<KeyCode> {
        let mut v = std::mem::take(&mut self.order);
        v.reverse();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [KeyCode; 41] = {
        use KeyCode::*;
        [
            Ctrl, Alt, Shift, Win, Esc, Tab, Enter, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1, F2, F3, F4,
            F5, F6, F7, F8, F9, F10, F11, F12, A, C, D, E, L, R, S, V, X, Y, Z,
        ]
    };

    #[test]
    fn every_key_has_unique_vk_and_scan_pair() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.spec().vk, b.spec().vk, "{a:?} ve {b:?} aynı sanal tuş");
                assert_ne!((a.spec().scan, a.spec().extended), (b.spec().scan, b.spec().extended), "{a:?} ve {b:?} aynı tarama kodu");
            }
        }
    }

    #[test]
    fn known_codes_and_extended_flags() {
        let s = KeyCode::Right.spec();
        assert_eq!((s.vk, s.scan, s.extended), (0x27, 0x4D, true));
        let s = KeyCode::Enter.spec();
        assert_eq!((s.vk, s.scan, s.extended), (0x0D, 0x1C, false));
        let s = KeyCode::Ctrl.spec();
        assert_eq!((s.vk, s.scan, s.extended), (0xA2, 0x1D, false));
        let s = KeyCode::Win.spec();
        assert_eq!((s.vk, s.scan, s.extended), (0x5B, 0x5B, true));
        assert_eq!(KeyCode::F12.spec().vk, 0x7B);
        assert_eq!(KeyCode::F11.spec().scan, 0x57);
        assert_eq!(KeyCode::Z.spec().vk, b'Z' as u16);
        for k in [KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right, KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown, KeyCode::Delete, KeyCode::Insert] {
            assert!(k.spec().extended, "{k:?} genişletilmiş olmalı");
        }
        for k in [KeyCode::Tab, KeyCode::Esc, KeyCode::Backspace, KeyCode::F5, KeyCode::C, KeyCode::Alt, KeyCode::Shift] {
            assert!(!k.spec().extended, "{k:?} genişletilmemiş olmalı");
        }
    }

    #[test]
    fn modifiers_are_flagged() {
        assert!(KeyCode::Ctrl.is_modifier() && KeyCode::Alt.is_modifier() && KeyCode::Shift.is_modifier() && KeyCode::Win.is_modifier());
        assert!(!KeyCode::Tab.is_modifier() && !KeyCode::C.is_modifier());
    }

    #[test]
    fn release_all_releases_in_reverse_and_clears() {
        let mut h = HeldKeys::default();
        h.down(KeyCode::Ctrl);
        h.down(KeyCode::Shift);
        h.down(KeyCode::Ctrl); // çift KeyDown tekrar eklemez
        h.down(KeyCode::Win);
        h.up(KeyCode::Shift);
        assert!(h.is_held(KeyCode::Ctrl) && !h.is_held(KeyCode::Shift));
        assert_eq!(h.release_all(), vec![KeyCode::Win, KeyCode::Ctrl]);
        assert!(h.is_empty());
        assert!(h.release_all().is_empty());
    }

    #[test]
    fn key_messages_roundtrip_and_cap_advertised() {
        use crate::proto::*;
        for m in [Msg::KeyDown { key: KeyCode::Ctrl }, Msg::KeyUp { key: KeyCode::F12 }] {
            let e = encode(&m).unwrap();
            assert_eq!(decode_msg(&e[4..]), Some(m));
        }
        assert!(local_caps().iter().any(|c| c == CAP_KEY_HOLD));
    }
}
