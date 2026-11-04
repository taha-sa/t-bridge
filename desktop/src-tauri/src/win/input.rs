//! Uzaktan kumanda: SendInput ile fare/klavye/medya, güç işlemleri.
use std::mem::size_of;

use kopru_core::{MediaAction, MouseButton, PcCommand, SpecialKey, VolumeAction};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs, size_of::<INPUT>() as i32);
    }
}

fn mouse(dx: i32, dy: i32, data: i32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn is_extended(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_INSERT | VK_DELETE
    )
}

fn unicode(unit: u16, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_UNICODE | KEYEVENTF_KEYUP } else { KEYEVENTF_UNICODE };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: unit, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn tap(vk: VIRTUAL_KEY) {
    send(&[key(vk, false), key(vk, true)]);
}

fn combo(mods: &[VIRTUAL_KEY], vk: VIRTUAL_KEY) {
    let mut v: Vec<INPUT> = mods.iter().map(|m| key(*m, false)).collect();
    v.push(key(vk, false));
    v.push(key(vk, true));
    v.extend(mods.iter().rev().map(|m| key(*m, true)));
    send(&v);
}

pub fn move_rel(dx: i32, dy: i32) {
    send(&[mouse(dx, dy, 0, MOUSEEVENTF_MOVE)]);
}

fn button_flags(b: MouseButton) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
    match b {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

/// Metni yazar: her UTF-16 birimi için Unicode tuş olayı (Türkçe karakterler klavye düzeninden bağımsız).
pub fn type_text(text: &str) {
    let mut v = Vec::with_capacity(text.len() * 2);
    let flush = |v: &mut Vec<INPUT>| {
        send(v);
        v.clear();
    };
    for ch in text.chars() {
        match ch {
            '\r' => {}
            '\n' => {
                flush(&mut v);
                tap(VK_RETURN);
            }
            '\t' => {
                flush(&mut v);
                tap(VK_TAB);
            }
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    v.push(unicode(*u, false));
                    v.push(unicode(*u, true));
                }
            }
        }
    }
    flush(&mut v);
}

fn special(k: SpecialKey) {
    match k {
        SpecialKey::Enter => tap(VK_RETURN),
        SpecialKey::Backspace => tap(VK_BACK),
        SpecialKey::Delete => tap(VK_DELETE),
        SpecialKey::Escape => tap(VK_ESCAPE),
        SpecialKey::Tab => tap(VK_TAB),
        SpecialKey::Up => tap(VK_UP),
        SpecialKey::Down => tap(VK_DOWN),
        SpecialKey::Left => tap(VK_LEFT),
        SpecialKey::Right => tap(VK_RIGHT),
        SpecialKey::Home => tap(VK_HOME),
        SpecialKey::End => tap(VK_END),
        SpecialKey::PageUp => tap(VK_PRIOR),
        SpecialKey::PageDown => tap(VK_NEXT),
        SpecialKey::Win => tap(VK_LWIN),
        SpecialKey::AltTab => combo(&[VK_MENU], VK_TAB),
        SpecialKey::CopyShortcut => combo(&[VK_CONTROL], VIRTUAL_KEY(b'C' as u16)),
        SpecialKey::PasteShortcut => combo(&[VK_CONTROL], VIRTUAL_KEY(b'V' as u16)),
        SpecialKey::F5 => tap(VK_F5),
    }
}

pub fn exec(cmd: &PcCommand) {
    match cmd {
        PcCommand::Button { button, down } => {
            let (d, u) = button_flags(*button);
            send(&[mouse(0, 0, 0, if *down { d } else { u })]);
        }
        PcCommand::Click { button, double } => {
            let (d, u) = button_flags(*button);
            let one = [mouse(0, 0, 0, d), mouse(0, 0, 0, u)];
            send(&one);
            if *double {
                send(&one);
            }
        }
        PcCommand::Scroll { dx, dy } => {
            let mut v = Vec::new();
            if *dy != 0 {
                v.push(mouse(0, 0, -*dy, MOUSEEVENTF_WHEEL));
            }
            if *dx != 0 {
                v.push(mouse(0, 0, *dx, MOUSEEVENTF_HWHEEL));
            }
            send(&v);
        }
        PcCommand::Text { text } => type_text(text),
        PcCommand::Key { key } => special(*key),
        PcCommand::Media { action } => tap(match action {
            MediaAction::PlayPause => VK_MEDIA_PLAY_PAUSE,
            MediaAction::Next => VK_MEDIA_NEXT_TRACK,
            MediaAction::Previous => VK_MEDIA_PREV_TRACK,
            MediaAction::Stop => VK_MEDIA_STOP,
        }),
        PcCommand::Volume { action } => tap(match action {
            VolumeAction::Up => VK_VOLUME_UP,
            VolumeAction::Down => VK_VOLUME_DOWN,
            VolumeAction::Mute => VK_VOLUME_MUTE,
        }),
        PcCommand::Power { action } => super::system::power(*action),
    }
}
