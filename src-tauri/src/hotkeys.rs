use crate::engine::worker::emit_status;
use crate::engine::worker::now_epoch_ms;
use crate::engine::worker::start_clicker_inner;
use crate::engine::worker::stop_clicker_inner;
use crate::engine::worker::toggle_clicker_inner;
use crate::engine::AUTOCLICKER_EXTRA_INFO;
use crate::error::poisoned_inner;
use crate::error::AppError;
use crate::error::AppResult;
use crate::AppHandle;
use crate::ClickerState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use tauri::Manager;
use windows_sys::Win32::Foundation::{GetLastError, LRESULT, POINT};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetCursorPos, GetWindowThreadProcessId, PeekMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, WaitMessage, WindowFromPoint, GA_ROOT, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

const PM_REMOVE: u32 = 0x0001;
const PM_NOREMOVE: u32 = 0x0000;

const POLL_INTERVAL: Duration = Duration::from_millis(4);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub main_vk: Option<i32>,
    pub key_token: String,
}

pub fn register_hotkey_inner(app: &AppHandle, hotkey: String) -> AppResult<String> {
    let state = app.state::<ClickerState>();
    state
        .suppress_hotkey_until_ms
        .store(now_epoch_ms().saturating_add(250), Ordering::SeqCst);
    state
        .suppress_hotkey_until_release
        .store(true, Ordering::SeqCst);

    if hotkey.is_empty() {
        *state
            .registered_hotkey
            .lock()
            .unwrap_or_else(poisoned_inner) = None;
        return Ok(String::new());
    }

    let binding = parse_hotkey_binding(&hotkey)?;
    *state
        .registered_hotkey
        .lock()
        .unwrap_or_else(poisoned_inner) = Some(binding.clone());

    Ok(format_hotkey_binding(&binding))
}

pub fn register_master_inner(app: &AppHandle, hotkey: String, hold_mode: bool) -> AppResult<()> {
    let state = app.state::<ClickerState>();
    let binding = if hotkey.is_empty() {
        None
    } else {
        Some(parse_hotkey_binding(&hotkey)?)
    };
    let prev_key = state
        .master_key
        .lock()
        .unwrap_or_else(poisoned_inner)
        .as_ref()
        .map(format_hotkey_binding);
    let new_key = binding.as_ref().map(format_hotkey_binding);
    let key_changed = prev_key != new_key;

    *state.master_key.lock().unwrap_or_else(poisoned_inner) = binding;
    state.master_hold_mode.store(hold_mode, Ordering::SeqCst);

    if new_key.is_none() {
        // Clearing the key always allows clicking; avoid a one-poll stale "off".
        state.master_allowed.store(true, Ordering::SeqCst);
        state.last_master_allowed.store(true, Ordering::SeqCst);
    } else if key_changed {
        // A freshly (re)bound key defaults to enabled, but a mode-only change
        // must not silently re-enable a master the user toggled off.
        let enabled = true;
        state.master_enabled.store(enabled, Ordering::SeqCst);
        state.master_allowed.store(enabled, Ordering::SeqCst);
        state.last_master_allowed.store(enabled, Ordering::SeqCst);
    }
    emit_status(app);
    Ok(())
}

pub fn normalize_hotkey(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn parse_hotkey_binding(hotkey: &str) -> AppResult<HotkeyBinding> {
    let normalized = normalize_hotkey(hotkey);
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_key = false;
    let mut main_key: Option<(i32, String)> = None;

    for token in normalized.split('+').map(str::trim) {
        if token.is_empty() {
            return Err(AppError::Hotkey(format!(
                "Invalid hotkey '{hotkey}': found empty key token"
            )));
        }

        match normalize_modifier_token(token) {
            Some("ctrl") => ctrl = true,
            Some("alt") => alt = true,
            Some("shift") => shift = true,
            Some("super") => super_key = true,
            Some(_) => {}
            None => {
                if main_key
                    .replace(parse_hotkey_main_key(token, hotkey)?)
                    .is_some()
                {
                    return Err(AppError::Hotkey(format!(
                        "Invalid hotkey '{hotkey}': use modifiers first and only one main key"
                    )));
                }
            }
        }
    }

    let main_vk = main_key.as_ref().map(|(vk, _)| *vk);
    let key_token = main_key
        .as_ref()
        .map(|(_, tok)| tok.clone())
        .unwrap_or_default();

    Ok(HotkeyBinding {
        ctrl,
        alt,
        shift,
        super_key,
        main_vk,
        key_token,
    })
}

pub fn parse_hotkey_main_key(token: &str, original_hotkey: &str) -> AppResult<(i32, String)> {
    let lower = token.trim().to_ascii_lowercase();

    if let Some(binding) = parse_named_key_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_mouse_button_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_numpad_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_function_key_token(&lower) {
        return Ok(binding);
    }

    if let Some(letter) = lower.strip_prefix("key") {
        if letter.len() == 1 {
            return parse_hotkey_main_key(letter, original_hotkey);
        }
    }

    if let Some(digit) = lower.strip_prefix("digit") {
        if digit.len() == 1 {
            return parse_hotkey_main_key(digit, original_hotkey);
        }
    }

    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch.to_ascii_uppercase() as i32, lower));
        }
        if ch.is_ascii_digit() {
            return Ok((ch as i32, lower));
        }
    }

    Err(AppError::Hotkey(format!(
        "Couldn't recognize '{token}' as a valid key in '{original_hotkey}'"
    )))
}

pub fn format_hotkey_binding(binding: &HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();

    if binding.ctrl {
        parts.push(String::from("ctrl"));
    }
    if binding.alt {
        parts.push(String::from("alt"));
    }
    if binding.shift {
        parts.push(String::from("shift"));
    }
    if binding.super_key {
        parts.push(String::from("super"));
    }

    if !binding.key_token.is_empty() {
        parts.push(binding.key_token.clone());
    }
    parts.join("+")
}

static PHYSICAL_KEY_STATE: OnceLock<&'static [AtomicBool; 256]> = OnceLock::new();
static HOOKS_ACTIVE: AtomicBool = AtomicBool::new(false);

fn physical_key_state() -> &'static [AtomicBool; 256] {
    PHYSICAL_KEY_STATE
        .get_or_init(|| Box::leak(Box::new(std::array::from_fn(|_| AtomicBool::new(false)))))
}

fn is_physical_vk_down(vk: i32) -> bool {
    if !(0..256).contains(&vk) {
        return false;
    }
    physical_key_state()[vk as usize].load(Ordering::Relaxed)
}

fn normalize_low_level_keyboard_vk(khs: &KBDLLHOOKSTRUCT) -> i32 {
    match khs.vkCode as u16 {
        VK_SHIFT => {
            let mapped = unsafe { MapVirtualKeyW(khs.scanCode, MAPVK_VSC_TO_VK_EX) };
            if mapped == 0 {
                VK_SHIFT as i32
            } else {
                mapped as i32
            }
        }
        VK_CONTROL => {
            if (khs.flags & LLKHF_EXTENDED) != 0 {
                VK_RCONTROL as i32
            } else {
                VK_LCONTROL as i32
            }
        }
        VK_MENU => {
            if (khs.flags & LLKHF_EXTENDED) != 0 {
                VK_RMENU as i32
            } else {
                VK_LMENU as i32
            }
        }
        _ => khs.vkCode as i32,
    }
}

unsafe extern "system" fn mouse_ll_proc(n_code: i32, w_param: usize, l_param: isize) -> LRESULT {
    if n_code >= 0 {
        let mhs = &*(l_param as *const MSLLHOOKSTRUCT);
        if (mhs.dwExtraInfo) != AUTOCLICKER_EXTRA_INFO {
            let (vk, down) = match w_param as u32 {
                WM_LBUTTONDOWN => (VK_LBUTTON as i32, true),
                WM_LBUTTONUP => (VK_LBUTTON as i32, false),
                WM_RBUTTONDOWN => (VK_RBUTTON as i32, true),
                WM_RBUTTONUP => (VK_RBUTTON as i32, false),
                WM_MBUTTONDOWN => (VK_MBUTTON as i32, true),
                WM_MBUTTONUP => (VK_MBUTTON as i32, false),
                WM_XBUTTONDOWN => {
                    let x = if ((mhs.mouseData >> 16) & 0xFFFF) == 1 {
                        VK_XBUTTON1 as i32
                    } else {
                        VK_XBUTTON2 as i32
                    };
                    (x, true)
                }
                WM_XBUTTONUP => {
                    let x = if ((mhs.mouseData >> 16) & 0xFFFF) == 1 {
                        VK_XBUTTON1 as i32
                    } else {
                        VK_XBUTTON2 as i32
                    };
                    (x, false)
                }
                _ => (-1, false),
            };
            if vk >= 0 && (vk as usize) < 256 {
                physical_key_state()[vk as usize].store(down, Ordering::Relaxed);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn keyboard_ll_proc(n_code: i32, w_param: usize, l_param: isize) -> LRESULT {
    if n_code >= 0 {
        let khs = &*(l_param as *const KBDLLHOOKSTRUCT);
        if (khs.dwExtraInfo) != AUTOCLICKER_EXTRA_INFO {
            let vk = normalize_low_level_keyboard_vk(khs);
            if (0..256).contains(&vk) {
                let down = matches!(w_param as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                physical_key_state()[vk as usize].store(down, Ordering::Relaxed);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

pub fn start_hotkey_listener(app: AppHandle) {
    std::thread::spawn(move || unsafe {
        // Delay hook installation to let WebView2/windows fully initialise.
        // Installing WH_KEYBOARD_LL too early can cause Windows to generate
        // spurious Alt-menu events in other applications. Hotkey detection
        // falls back to GetAsyncKeyState via the HOOKS_ACTIVE check below
        // during this window.
        std::thread::sleep(Duration::from_secs(2));

        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_ll_proc), std::ptr::null_mut(), 0);
        let kb_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_ll_proc),
            std::ptr::null_mut(),
            0,
        );

        if !mouse_hook.is_null() && !kb_hook.is_null() {
            HOOKS_ACTIVE.store(true, Ordering::SeqCst);
        } else {
            let err = GetLastError();
            log::warn!("[Hotkeys] {}", AppError::WindowsSystem(err));
        }

        let state = app.state::<ClickerState>();
        let mut was_pressed = false;
        let mut was_suppressed = false;
        let mut master_was_pressed = false;
        let mut last_check = Instant::now();
        let mut msg: MSG = std::mem::zeroed();

        'outer: loop {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
            }

            if last_check.elapsed() >= POLL_INTERVAL {
                last_check = Instant::now();

                let (binding, strict) = {
                    let binding = state
                        .registered_hotkey
                        .lock()
                        .unwrap_or_else(poisoned_inner)
                        .clone();
                    let strict = state
                        .settings
                        .lock()
                        .unwrap_or_else(poisoned_inner)
                        .strict_hotkey_modifiers;
                    (binding, strict)
                };

                let running = state.running.load(Ordering::SeqCst);
                let currently_pressed = binding
                    .as_ref()
                    .map(|b| {
                        if HOOKS_ACTIVE.load(Ordering::Relaxed) {
                            let physical = is_hotkey_binding_pressed_physical(b, strict);
                            physical || (!running && is_hotkey_binding_pressed(b, strict))
                        } else {
                            is_hotkey_binding_pressed(b, strict)
                        }
                    })
                    .unwrap_or(false);

                let master_binding = {
                    state
                        .master_key
                        .lock()
                        .unwrap_or_else(poisoned_inner)
                        .clone()
                };
                let master_hold = state.master_hold_mode.load(Ordering::SeqCst);
                let mut master_enabled = state.master_enabled.load(Ordering::SeqCst);

                let master_binding_pressed = match master_binding {
                    None => false,
                    Some(ref b) => {
                        if HOOKS_ACTIVE.load(Ordering::Relaxed) {
                            is_hotkey_binding_pressed_physical(b, strict)
                                || (!running && is_hotkey_binding_pressed(b, strict))
                        } else {
                            is_hotkey_binding_pressed(b, strict)
                        }
                    }
                };

                if !master_hold && master_binding_pressed && !master_was_pressed {
                    master_enabled = !master_enabled;
                    state.master_enabled.store(master_enabled, Ordering::SeqCst);
                }
                master_was_pressed = master_binding_pressed;

                let master_allowed = match master_binding {
                    None => true,
                    Some(_) => {
                        if master_hold {
                            master_binding_pressed
                        } else {
                            master_enabled
                        }
                    }
                };

                if master_allowed != state.last_master_allowed.load(Ordering::SeqCst) {
                    state.master_allowed.store(master_allowed, Ordering::SeqCst);
                    state
                        .last_master_allowed
                        .store(master_allowed, Ordering::SeqCst);
                    emit_status(&app);
                }

                if running && !master_allowed {
                    let _ =
                        stop_clicker_inner(&app, Some(String::from("Stopped by master switch")));
                }

                let suppress_until = state.suppress_hotkey_until_ms.load(Ordering::SeqCst);
                let suppress_until_release =
                    state.suppress_hotkey_until_release.load(Ordering::SeqCst);
                let hotkey_capture_active = state.hotkey_capture_active.load(Ordering::SeqCst);
                let click_point_pick_active = state.click_point_pick_active.load(Ordering::SeqCst);
                let custom_stop_zone_pick_active =
                    state.custom_stop_zone_pick_active.load(Ordering::SeqCst);

                if hotkey_capture_active || click_point_pick_active || custom_stop_zone_pick_active
                {
                    if currently_pressed && !was_pressed && hotkey_capture_active {
                        let needs_emit = {
                            let mut warning = state.warning.lock().unwrap_or_else(poisoned_inner);
                            if warning.is_none() {
                                *warning = Some(String::from("Finish setting hotkey first"));
                                true
                            } else {
                                false
                            }
                        };
                        if needs_emit {
                            emit_status(&app);
                        }
                    }
                    was_pressed = currently_pressed;
                    continue;
                }

                if suppress_until_release {
                    if currently_pressed {
                        was_pressed = true;
                        continue;
                    }
                    state
                        .suppress_hotkey_until_release
                        .store(false, Ordering::SeqCst);
                    was_pressed = false;
                    was_suppressed = false;
                    continue;
                }

                if now_epoch_ms() < suppress_until {
                    was_pressed = currently_pressed;
                    continue;
                }

                let suppress_mouse_on_own_window =
                    binding.as_ref().is_some_and(is_mouse_hotkey_binding)
                        && is_cursor_over_own_window();

                if currently_pressed && !was_pressed {
                    if suppress_mouse_on_own_window {
                        was_suppressed = true;
                    } else {
                        was_suppressed = false;
                        if master_allowed {
                            handle_hotkey_pressed(&app);
                        }
                    }
                } else if !currently_pressed && was_pressed {
                    if !was_suppressed {
                        handle_hotkey_released(&app);
                    }
                    was_suppressed = false;
                }

                was_pressed = currently_pressed;
            } else if HOOKS_ACTIVE.load(Ordering::Relaxed) {
                if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) == 0 {
                    WaitMessage();
                }
            } else {
                std::thread::sleep(POLL_INTERVAL);
            }
        }

        if !mouse_hook.is_null() {
            UnhookWindowsHookEx(mouse_hook);
        }
        if !kb_hook.is_null() {
            UnhookWindowsHookEx(kb_hook);
        }
    });
}

fn is_mouse_hotkey_binding(binding: &HotkeyBinding) -> bool {
    [
        VK_LBUTTON as i32,
        VK_RBUTTON as i32,
        VK_MBUTTON as i32,
        VK_XBUTTON1 as i32,
        VK_XBUTTON2 as i32,
    ]
    .contains(&binding.main_vk.unwrap_or(-1))
}

fn is_cursor_over_own_window() -> bool {
    unsafe {
        let mut pt: POINT = std::mem::zeroed();
        if GetCursorPos(&mut pt) == 0 {
            return false;
        }
        let hwnd = WindowFromPoint(pt);
        if hwnd.is_null() {
            return false;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let root = if root.is_null() { hwnd } else { root };
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(root, &mut pid);
        pid == GetCurrentProcessId()
    }
}

fn is_hotkey_binding_pressed_physical(binding: &HotkeyBinding, strict: bool) -> bool {
    let ctrl_down =
        is_physical_vk_down(VK_LCONTROL as i32) || is_physical_vk_down(VK_RCONTROL as i32);
    let alt_down = is_physical_vk_down(VK_LMENU as i32) || is_physical_vk_down(VK_RMENU as i32);
    let shift_down = is_physical_vk_down(VK_LSHIFT as i32) || is_physical_vk_down(VK_RSHIFT as i32);
    let super_down = is_physical_vk_down(VK_LWIN as i32) || is_physical_vk_down(VK_RWIN as i32);
    if !modifiers_match(binding, ctrl_down, alt_down, shift_down, super_down, strict) {
        return false;
    }
    match binding.main_vk {
        Some(vk) => is_physical_vk_down(vk),
        None => true,
    }
}

pub fn handle_hotkey_pressed(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let mode = state
            .settings
            .lock()
            .unwrap_or_else(poisoned_inner)
            .mode
            .clone();
        mode
    };

    if mode == "Toggle" {
        if let Err(e) = toggle_clicker_inner(app) {
            log::error!("[Hotkey] Toggle failed: {e}");
        }
    } else if mode == "Hold" {
        if let Err(e) = start_clicker_inner(app) {
            log::error!("[Hotkey] Start failed: {e}");
        }
    }
}

pub fn handle_hotkey_released(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let mode = state
            .settings
            .lock()
            .unwrap_or_else(poisoned_inner)
            .mode
            .clone();
        mode
    };

    if mode == "Hold" {
        if let Err(e) = stop_clicker_inner(app, Some(String::from("Stopped from hold hotkey"))) {
            log::error!("[Hotkey] Stop failed: {e}");
        }
    }
}

pub fn is_hotkey_binding_pressed(binding: &HotkeyBinding, strict: bool) -> bool {
    let ctrl_down = is_vk_down(VK_CONTROL as i32);
    let alt_down = is_vk_down(VK_MENU as i32);
    let shift_down = is_vk_down(VK_SHIFT as i32);
    let super_down = is_vk_down(VK_LWIN as i32) || is_vk_down(VK_RWIN as i32);

    if !modifiers_match(binding, ctrl_down, alt_down, shift_down, super_down, strict) {
        return false;
    }

    match binding.main_vk {
        Some(vk) => is_vk_down(vk),
        None => true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModifierGroup {
    Ctrl,
    Alt,
    Shift,
    Super,
}

fn modifier_group_for_vk(vk: i32) -> Option<ModifierGroup> {
    if [VK_CONTROL as i32, VK_LCONTROL as i32, VK_RCONTROL as i32].contains(&vk) {
        Some(ModifierGroup::Ctrl)
    } else if [VK_MENU as i32, VK_LMENU as i32, VK_RMENU as i32].contains(&vk) {
        Some(ModifierGroup::Alt)
    } else if [VK_SHIFT as i32, VK_LSHIFT as i32, VK_RSHIFT as i32].contains(&vk) {
        Some(ModifierGroup::Shift)
    } else if [VK_LWIN as i32, VK_RWIN as i32].contains(&vk) {
        Some(ModifierGroup::Super)
    } else {
        None
    }
}

fn modifiers_match(
    binding: &HotkeyBinding,
    ctrl_down: bool,
    alt_down: bool,
    shift_down: bool,
    super_down: bool,
    strict: bool,
) -> bool {
    if binding.ctrl && !ctrl_down {
        return false;
    }
    if binding.alt && !alt_down {
        return false;
    }
    if binding.shift && !shift_down {
        return false;
    }
    if binding.super_key && !super_down {
        return false;
    }

    if strict {
        let main_modifier_group = binding.main_vk.and_then(modifier_group_for_vk);
        if ctrl_down && !binding.ctrl && main_modifier_group != Some(ModifierGroup::Ctrl) {
            return false;
        }
        if alt_down && !binding.alt && main_modifier_group != Some(ModifierGroup::Alt) {
            return false;
        }
        if shift_down && !binding.shift && main_modifier_group != Some(ModifierGroup::Shift) {
            return false;
        }
        if super_down && !binding.super_key && main_modifier_group != Some(ModifierGroup::Super) {
            return false;
        }
    }

    true
}

pub fn is_vk_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

fn normalize_modifier_token(token: &str) -> Option<&'static str> {
    match token {
        "alt" | "option" | "leftalt" | "rightalt" | "altleft" | "altright" | "lalt" | "ralt" => {
            Some("alt")
        }
        "ctrl" | "control" | "leftctrl" | "rightctrl" | "ctrlleft" | "ctrlright" | "lctrl"
        | "rctrl" => Some("ctrl"),
        "shift" | "leftshift" | "rightshift" | "shiftleft" | "shiftright" | "lshift" | "rshift" => {
            Some("shift")
        }
        "super" | "command" | "cmd" | "meta" | "win" | "leftsuper" | "rightsuper" | "superleft"
        | "superright" | "leftwin" | "rightwin" | "winleft" | "winright" | "lwin" | "rwin" => {
            Some("super")
        }
        _ => None,
    }
}

fn binding(vk: i32, token: &str) -> (i32, String) {
    (vk, token.to_string())
}

fn parse_named_key_token(token: &str) -> Option<(i32, String)> {
    match token {
        "<" | ">" | "intlbackslash" | "oem102" | "nonusbackslash" => {
            Some(binding(VK_OEM_102 as i32, "IntlBackslash"))
        }
        "space" | "spacebar" => Some(binding(VK_SPACE as i32, "space")),
        "tab" => Some(binding(VK_TAB as i32, "tab")),
        "enter" | "return" => Some(binding(VK_RETURN as i32, "enter")),
        "backspace" => Some(binding(VK_BACK as i32, "backspace")),
        "delete" | "del" => Some(binding(VK_DELETE as i32, "delete")),
        "insert" | "ins" => Some(binding(VK_INSERT as i32, "insert")),
        "home" => Some(binding(VK_HOME as i32, "home")),
        "end" => Some(binding(VK_END as i32, "end")),
        "pageup" | "pgup" => Some(binding(VK_PRIOR as i32, "pageup")),
        "pagedown" | "pgdn" => Some(binding(VK_NEXT as i32, "pagedown")),
        "up" | "arrowup" => Some(binding(VK_UP as i32, "up")),
        "down" | "arrowdown" => Some(binding(VK_DOWN as i32, "down")),
        "left" | "arrowleft" => Some(binding(VK_LEFT as i32, "left")),
        "right" | "arrowright" => Some(binding(VK_RIGHT as i32, "right")),
        "esc" | "escape" => Some(binding(VK_ESCAPE as i32, "escape")),
        "leftctrl" | "ctrlleft" | "lctrl" => Some(binding(VK_LCONTROL as i32, "leftctrl")),
        "rightctrl" | "ctrlright" | "rctrl" => Some(binding(VK_RCONTROL as i32, "rightctrl")),
        "leftshift" | "shiftleft" | "lshift" => Some(binding(VK_LSHIFT as i32, "leftshift")),
        "rightshift" | "shiftright" | "rshift" => Some(binding(VK_RSHIFT as i32, "rightshift")),
        "leftalt" | "altleft" | "lalt" => Some(binding(VK_LMENU as i32, "leftalt")),
        "rightalt" | "altright" | "ralt" | "altgr" => Some(binding(VK_RMENU as i32, "rightalt")),
        "leftsuper" | "superleft" | "leftwin" | "winleft" | "lwin" => {
            Some(binding(VK_LWIN as i32, "leftsuper"))
        }
        "rightsuper" | "superright" | "rightwin" | "winright" | "rwin" => {
            Some(binding(VK_RWIN as i32, "rightsuper"))
        }
        "capslock" => Some(binding(VK_CAPITAL as i32, "capslock")),
        "numlock" => Some(binding(VK_NUMLOCK as i32, "numlock")),
        "scrolllock" => Some(binding(VK_SCROLL as i32, "scrolllock")),
        "menu" | "apps" | "contextmenu" => Some(binding(VK_APPS as i32, "menu")),
        "printscreen" | "prtsc" | "snapshot" => Some(binding(VK_SNAPSHOT as i32, "printscreen")),
        "pause" | "break" => Some(binding(VK_PAUSE as i32, "pause")),
        "/" | "slash" => Some(binding(VK_OEM_2 as i32, "/")),
        "\\" | "backslash" => Some(binding(VK_OEM_5 as i32, "\\")),
        ";" | "semicolon" => Some(binding(VK_OEM_1 as i32, ";")),
        "'" | "quote" | "apostrophe" => Some(binding(VK_OEM_7 as i32, "'")),
        "[" | "bracketleft" => Some(binding(VK_OEM_4 as i32, "[")),
        "]" | "bracketright" => Some(binding(VK_OEM_6 as i32, "]")),
        "-" | "minus" => Some(binding(VK_OEM_MINUS as i32, "-")),
        "=" | "equal" => Some(binding(VK_OEM_PLUS as i32, "=")),
        "`" | "backquote" | "grave" => Some(binding(VK_OEM_3 as i32, "`")),
        "," | "comma" => Some(binding(VK_OEM_COMMA as i32, ",")),
        "." | "period" | "dot" => Some(binding(VK_OEM_PERIOD as i32, ".")),
        _ => None,
    }
}

fn parse_mouse_button_token(token: &str) -> Option<(i32, String)> {
    match token {
        "mouseleft" | "leftmouse" | "leftbutton" | "mouse1" | "lmb" => {
            Some(binding(VK_LBUTTON as i32, "mouseleft"))
        }
        "mouseright" | "rightmouse" | "rightbutton" | "mouse2" | "rmb" => {
            Some(binding(VK_RBUTTON as i32, "mouseright"))
        }
        "mousemiddle" | "middlemouse" | "middlebutton" | "mouse3" | "mmb" | "scrollbutton"
        | "middleclick" => Some(binding(VK_MBUTTON as i32, "mousemiddle")),
        "mouse4" | "xbutton1" | "mouseback" | "browserback" | "backbutton" => {
            Some(binding(VK_XBUTTON1 as i32, "mouse4"))
        }
        "mouse5" | "xbutton2" | "mouseforward" | "browserforward" | "forwardbutton" => {
            Some(binding(VK_XBUTTON2 as i32, "mouse5"))
        }
        _ => None,
    }
}

fn parse_numpad_token(token: &str) -> Option<(i32, String)> {
    match token {
        "numpad0" | "num0" => Some(binding(VK_NUMPAD0 as i32, "numpad0")),
        "numpad1" | "num1" => Some(binding(VK_NUMPAD1 as i32, "numpad1")),
        "numpad2" | "num2" => Some(binding(VK_NUMPAD2 as i32, "numpad2")),
        "numpad3" | "num3" => Some(binding(VK_NUMPAD3 as i32, "numpad3")),
        "numpad4" | "num4" => Some(binding(VK_NUMPAD4 as i32, "numpad4")),
        "numpad5" | "num5" => Some(binding(VK_NUMPAD5 as i32, "numpad5")),
        "numpad6" | "num6" => Some(binding(VK_NUMPAD6 as i32, "numpad6")),
        "numpad7" | "num7" => Some(binding(VK_NUMPAD7 as i32, "numpad7")),
        "numpad8" | "num8" => Some(binding(VK_NUMPAD8 as i32, "numpad8")),
        "numpad9" | "num9" => Some(binding(VK_NUMPAD9 as i32, "numpad9")),
        "numpadadd" | "numadd" | "numpadplus" | "numplus" => {
            Some(binding(VK_ADD as i32, "numpadadd"))
        }
        "numpadsubtract" | "numsubtract" | "numsub" | "numpadminus" | "numminus" => {
            Some(binding(VK_SUBTRACT as i32, "numpadsubtract"))
        }
        "numpadmultiply" | "nummultiply" | "nummul" | "numpadmul" => {
            Some(binding(VK_MULTIPLY as i32, "numpadmultiply"))
        }
        "numpaddivide" | "numdivide" | "numdiv" | "numpaddiv" => {
            Some(binding(VK_DIVIDE as i32, "numpaddivide"))
        }
        "numpaddecimal" | "numdecimal" | "numdot" | "numdel" | "numpadpoint" => {
            Some(binding(VK_DECIMAL as i32, "numpaddecimal"))
        }
        _ => None,
    }
}

fn parse_function_key_token(token: &str) -> Option<(i32, String)> {
    if !token.starts_with('f') || token.len() > 3 {
        return None;
    }

    let number = token[1..].parse::<i32>().ok()?;
    let vk = match number {
        1..=24 => VK_F1 as i32 + (number - 1),
        _ => return None,
    };

    Some(binding(vk, token))
}

#[cfg(test)]
mod tests {
    use super::{
        format_hotkey_binding, is_mouse_hotkey_binding, modifiers_match, parse_hotkey_binding,
    };

    #[test]
    fn numpad_tokens_round_trip() {
        for token in [
            "numpad0",
            "numpad1",
            "numpad2",
            "numpad3",
            "numpad4",
            "numpad5",
            "numpad6",
            "numpad7",
            "numpad8",
            "numpad9",
            "numpadadd",
            "numpadsubtract",
            "numpadmultiply",
            "numpaddivide",
            "numpaddecimal",
        ] {
            let hotkey = format!("ctrl+shift+{token}");
            let binding = parse_hotkey_binding(&hotkey).expect("token should parse");
            assert_eq!(binding.key_token, token);
            assert_eq!(format_hotkey_binding(&binding), hotkey);
        }
    }

    #[test]
    fn mouse_button_bindings_are_detected() {
        for hotkey in [
            "mouseleft",
            "mouseright",
            "mousemiddle",
            "mouse4",
            "mouse5",
            "ctrl+mouseleft",
            "shift+mouse4",
        ] {
            let binding = parse_hotkey_binding(hotkey).expect("mouse token should parse");
            assert!(is_mouse_hotkey_binding(&binding));
        }
        for hotkey in ["f8", "space", "ctrl+shift+f8"] {
            let binding = parse_hotkey_binding(hotkey).expect("key token should parse");
            assert!(!is_mouse_hotkey_binding(&binding));
        }
    }

    #[test]
    fn empty_hotkeys_are_rejected() {
        assert!(parse_hotkey_binding("").is_err());
        assert!(parse_hotkey_binding("ctrl+").is_err());
    }

    #[test]
    fn standalone_modifier_tokens_round_trip() {
        let cases: [(&str, bool, bool, bool, bool, &str); 8] = [
            ("leftctrl", true, false, false, false, "ctrl"),
            ("rightctrl", true, false, false, false, "ctrl"),
            ("leftshift", false, false, true, false, "shift"),
            ("rightshift", false, false, true, false, "shift"),
            ("leftalt", false, true, false, false, "alt"),
            ("rightalt", false, true, false, false, "alt"),
            ("leftsuper", false, false, false, true, "super"),
            ("rightsuper", false, false, false, true, "super"),
        ];
        for (token, ctrl, alt, shift, super_key, expected) in cases {
            let binding = parse_hotkey_binding(token).expect("modifier key should parse");
            assert_eq!(binding.main_vk, None, "token {token}");
            assert_eq!(binding.ctrl, ctrl, "token {token}");
            assert_eq!(binding.alt, alt, "token {token}");
            assert_eq!(binding.shift, shift, "token {token}");
            assert_eq!(binding.super_key, super_key, "token {token}");
            assert_eq!(format_hotkey_binding(&binding), expected, "token {token}");
        }
    }

    #[test]
    fn standalone_modifier_main_key_is_not_extra_in_strict_mode() {
        let binding = parse_hotkey_binding("leftalt").expect("left alt should parse");
        assert!(modifiers_match(&binding, false, true, false, false, true));
        assert!(!modifiers_match(&binding, true, true, false, false, true));
    }

    #[test]
    fn extra_modifiers_do_not_block_hotkeys_in_relaxed_mode() {
        let binding = parse_hotkey_binding("f11").expect("hotkey should parse");
        assert!(modifiers_match(&binding, false, false, true, false, false));
        assert!(modifiers_match(&binding, true, true, true, true, false));
    }

    #[test]
    fn extra_modifiers_block_hotkeys_in_strict_mode() {
        let binding = parse_hotkey_binding("f11").expect("hotkey should parse");
        assert!(!modifiers_match(&binding, false, false, true, false, true));
        assert!(!modifiers_match(&binding, true, true, true, true, true));
        assert!(modifiers_match(&binding, false, false, false, false, true));
    }
}
