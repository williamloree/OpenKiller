//! Dark modal dialogs (kill confirmation, favorite form with preset library, RAM cleaning, generic
//! confirmation) and the native "Save as" dialog.

use super::theme::{self, Kind, px};
use crate::memory::MemInfo;
use crate::model::{self, Favorite, PRESETS, fmt_gb};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, SetFocus, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, w};

pub const CLASS: &str = "OpenKillerDialog";
const OK_ID: i32 = 1;
const CANCEL_ID: i32 = 2;
const SHOT_TIMER: usize = 2;

type Handler = Box<dyn FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>;

thread_local! {
    static RESULT: Cell<Option<i32>> = const { Cell::new(None) };
    static HANDLER: RefCell<Option<Handler>> = RefCell::new(None);
    /// --screenshot: capture owner + dialog once shown, then close.
    pub static SHOT: RefCell<Option<(PathBuf, HWND)>> = const { RefCell::new(None) };
    pub static SHOT_OK: Cell<bool> = const { Cell::new(false) };
}

pub fn close(code: i32) {
    RESULT.with(|r| r.set(Some(code)));
}

pub unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let Some(r) = theme::ctl_color(msg, wp, lp, theme::SURFACE) {
        return r;
    }
    match msg {
        WM_DRAWITEM => {
            theme::draw_button(unsafe { &*(lp.0 as *const DRAWITEMSTRUCT) });
            return LRESULT(1);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_ACTIVATE if theme::keep_focus(h, wp) => return LRESULT(0),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(h, &mut ps);
                theme::fill(hdc, &ps.rcPaint, theme::SURFACE);
                theme::paint_edit_frames(h, hdc);
                let _ = EndPaint(h, &ps);
            }
            return LRESULT(0);
        }
        WM_CLOSE => {
            close(CANCEL_ID);
            return LRESULT(0);
        }
        WM_TIMER if wp.0 == SHOT_TIMER => {
            unsafe {
                let _ = KillTimer(Some(h), SHOT_TIMER);
            }
            if let Some((path, owner)) = SHOT.with(|s| s.borrow_mut().take()) {
                SHOT_OK.with(|ok| ok.set(super::shot::capture(owner, Some(h), &path).is_ok()));
            }
            close(CANCEL_ID);
            return LRESULT(0);
        }
        WM_COMMAND | WM_NOTIFY => {
            if msg == WM_COMMAND
                && matches!(((wp.0 >> 16) & 0xffff) as u32, EN_SETFOCUS | EN_KILLFOCUS)
            {
                unsafe {
                    let _ = InvalidateRect(Some(h), None, false);
                }
            }
            // Taken out while running so a re-entrant message cannot alias it.
            if let Some(mut f) = HANDLER.with(|c| c.borrow_mut().take()) {
                let r = f(h, msg, wp, lp);
                HANDLER.with(|c| *c.borrow_mut() = Some(f));
                if let Some(r) = r {
                    return r;
                }
            }
        }
        _ => {}
    }
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

/// Dialog under construction: controls placed in DIPs.
struct Builder {
    hwnd: HWND,
}

impl Builder {
    fn place(&self, h: HWND, x: i32, y: i32, w: i32, hh: i32) -> HWND {
        unsafe {
            let _ = MoveWindow(h, px(x), px(y), px(w), px(hh), false);
        }
        h
    }
    fn label(&self, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
        self.place(
            theme::label(self.hwnd, 0, text, WINDOW_STYLE(0)),
            x,
            y,
            w,
            h,
        )
    }
    fn title(&self, text: &str) {
        let l = self.label(text, 20, 16, 440, 26);
        theme::set_font(l, theme::bold());
    }
    fn edit(&self, id: i32, text: &str, x: i32, y: i32, w: i32, extra: WINDOW_STYLE) -> HWND {
        let h = theme::edit(self.hwnd, id, text, extra);
        theme::place_edit(h, px(x), px(y), px(w), px(30), false);
        h
    }
    fn button(&self, id: i32, text: &str, kind: Kind, x: i32, y: i32, w: i32) -> HWND {
        self.place(theme::button(self.hwnd, id, text, kind), x, y, w, 32)
    }
    /// Cancel + action buttons, bottom right of a `w` x `h` dialog.
    fn buttons(&self, w: i32, h: i32, ok: &str, kind: Kind) -> (HWND, HWND) {
        let cancel = self.button(CANCEL_ID, "Annuler", Kind::Normal, w - 330, h - 52, 120);
        let ok = self.button(OK_ID, ok, kind, w - 200, h - 52, 180);
        (cancel, ok)
    }
}

/// Runs a modal dialog of `w` x `h` DIPs (client area); returns the code given to close().
fn modal(
    owner: HWND,
    caption: &str,
    w: i32,
    h: i32,
    build: impl FnOnce(&Builder) -> HWND,
    handler: Handler,
) -> i32 {
    unsafe {
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: px(w),
            bottom: px(h),
        };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WS_EX_DLGMODALFRAME, theme::dpi());
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let mut or = RECT::default();
        let _ = GetWindowRect(owner, &mut or);
        let (x, y) = (
            or.left + (or.right - or.left - ww) / 2,
            or.top + (or.bottom - or.top - wh) / 2,
        );
        let Ok(dlg) = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            &HSTRING::from(CLASS),
            &HSTRING::from(caption),
            style,
            x,
            y.max(or.top),
            ww,
            wh,
            Some(owner),
            None,
            None,
            None,
        ) else {
            return CANCEL_ID;
        };
        theme::dark_title(dlg);
        let focus = build(&Builder { hwnd: dlg });
        RESULT.with(|r| r.set(None));
        HANDLER.with(|c| *c.borrow_mut() = Some(handler));
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetFocus(Some(focus));
        if SHOT.with(|s| s.borrow().is_some()) {
            SetTimer(Some(dlg), SHOT_TIMER, 700, None);
        }
        let mut msg = MSG::default();
        while RESULT.with(|r| r.get()).is_none() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            if msg.message == WM_KEYDOWN && (msg.hwnd == dlg || IsChild(dlg, msg.hwnd).as_bool()) {
                let key = msg.wParam.0 as u16;
                if key == VK_ESCAPE.0 {
                    close(CANCEL_ID);
                    continue;
                }
                if key == VK_RETURN.0 {
                    // Enter clicks the focused button (Cancel stays Cancel), otherwise validates.
                    let f = GetFocus();
                    if is_button(f) {
                        theme::send(f, BM_CLICK, 0, 0);
                    } else {
                        let _ = SendMessageW(dlg, WM_COMMAND, Some(WPARAM(OK_ID as usize)), None);
                    }
                    continue;
                }
            }
            if !IsDialogMessageW(dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        HANDLER.with(|c| c.borrow_mut().take());
        let _ = EnableWindow(owner, true);
        let _ = DestroyWindow(dlg);
        let _ = SetForegroundWindow(owner);
        RESULT.with(|r| r.take()).unwrap_or(CANCEL_ID)
    }
}

pub fn is_button(h: HWND) -> bool {
    let mut buf = [0u16; 16];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n as usize]).eq_ignore_ascii_case("button")
}

fn command_id(msg: u32, wp: WPARAM) -> Option<i32> {
    (msg == WM_COMMAND).then_some((wp.0 & 0xffff) as i32)
}

/// Closes on OK/Cancel; nothing else to handle.
fn ok_cancel() -> Handler {
    Box::new(|_, msg, wp, _| {
        match command_id(msg, wp) {
            Some(id @ (OK_ID | CANCEL_ID)) => close(id),
            _ => return None,
        }
        Some(LRESULT(0))
    })
}

/// One target of a kill: PID, name and the ports it holds.
pub type Target = (u32, String, Vec<u16>);

pub fn confirm_kill(owner: HWND, targets: &[Target]) -> bool {
    let ports = |p: &[u16]| p.iter().map(u16::to_string).collect::<Vec<_>>().join(", ");
    let (title, question, details, ok) = if let [(pid, name, p)] = targets {
        (
            "Terminer le processus ?".to_string(),
            "Êtes-vous sûr de vouloir terminer ce processus ?".to_string(),
            format!("Port : {}\nPID : {pid}\nApplication : {name}", ports(p)),
            "Terminer le processus".to_string(),
        )
    } else {
        let mut lines: Vec<String> = targets
            .iter()
            .take(8)
            .map(|(pid, name, p)| format!("PID {pid}  ·  {name}  ·  port {}", ports(p)))
            .collect();
        if targets.len() > 8 {
            lines.push(format!("… et {} autres", targets.len() - 8));
        }
        (
            format!("Terminer {} processus ?", targets.len()),
            format!(
                "Êtes-vous sûr de vouloir terminer ces {} processus ?",
                targets.len()
            ),
            lines.join("\n"),
            format!("Terminer les {}", targets.len()),
        )
    };
    let n_lines = details.lines().count() as i32;
    let (w, h) = (480, 196 + n_lines * 20);
    let code = modal(
        owner,
        "OpenKiller",
        w,
        h,
        |b| {
            b.title(&title);
            b.label(&question, 20, 50, w - 40, 22);
            let d = b.label(&details, 32, 80, w - 52, n_lines * 20 + 4);
            theme::set_label_color(d, theme::TEXT);
            let note = b.label(
                "Arrêt forcé (comme taskkill /F) : les données non enregistrées sont perdues.",
                20,
                92 + n_lines * 20,
                w - 40,
                36,
            );
            theme::set_label_color(note, theme::DIM);
            // Focus on Cancel: Enter alone never kills.
            b.buttons(w, h, &ok, Kind::Danger).0
        },
        ok_cancel(),
    );
    code == OK_ID
}

pub fn confirm(owner: HWND, title: &str, message: &str, ok: &str, kind: Kind) -> bool {
    let (w, h) = (440, 160);
    modal(
        owner,
        "OpenKiller",
        w,
        h,
        |b| {
            b.title(title);
            b.label(message, 20, 52, w - 40, 40);
            b.buttons(w, h, ok, kind).1
        },
        ok_cancel(),
    ) == OK_ID
}

pub fn confirm_clean(owner: HWND, m: &MemInfo) -> bool {
    let (w, h) = (480, 250);
    let ok = format!("Vider ~{}", fmt_gb(m.freeable));
    modal(
        owner,
        "OpenKiller",
        w,
        h,
        |b| {
            b.title("Vider la RAM");
            b.label(
                "Libère la mémoire physique que les applications ne réutilisent pas tout de suite, sans fermer \
                 de processus. Windows demandera une confirmation (UAC).",
                20,
                50,
                w - 40,
                54,
            );
            let d = b.label(
                &format!(
                    "RAM totale : {}\nRAM utilisée : {}\nRAM libérable estimée : ~{}",
                    fmt_gb(m.total),
                    fmt_gb(m.used),
                    fmt_gb(m.freeable)
                ),
                32,
                112,
                w - 52,
                66,
            );
            theme::set_label_color(d, theme::TEXT);
            b.buttons(w, h, &ok, Kind::Primary).1
        },
        ok_cancel(),
    ) == OK_ID
}

const ID_PORT: i32 = 201;
const ID_NAME: i32 = 202;
const ID_DESC: i32 = 203;
const ID_PRESETS: i32 = 204;

/// Add (`editing` = None, with the preset library) or edit a favorite. Returns the validated favorite.
pub fn favorite(
    owner: HWND,
    favorites: &[Favorite],
    editing: Option<&Favorite>,
) -> Option<Favorite> {
    let library = editing.is_none();
    let w = 520;
    let top = if library { 392 } else { 56 };
    let h = top + 236;
    let out: Rc<RefCell<Option<Favorite>>> = Rc::default();
    let edits: Rc<Cell<[HWND; 4]>> = Rc::default();
    let edits_b = edits.clone();
    let favs = favorites.to_vec();
    let taken: Vec<bool> = PRESETS
        .iter()
        .map(|(p, _)| favorites.iter().any(|f| f.port == *p))
        .collect();
    let taken_h = taken.clone();
    let out_h = out.clone();
    let editing_port = editing.map(|f| f.port);
    let handler: Handler = Box::new(move |_, msg, wp, lp| {
        let [port, name, desc, error] = edits.get();
        if msg == WM_NOTIFY {
            let hdr = unsafe { &*(lp.0 as *const NMHDR) };
            if hdr.idFrom != ID_PRESETS as usize {
                return None;
            }
            if hdr.code == NM_CUSTOMDRAW {
                let t = &taken_h;
                return Some(theme::lv_custom_draw(lp, |i, _| {
                    t.get(i as usize).filter(|&&x| x).map(|_| theme::FAINT)
                }));
            }
            if hdr.code == LVN_ITEMCHANGED {
                let nm = unsafe { &*(lp.0 as *const NMLISTVIEW) };
                if nm.uNewState & LVIS_SELECTED.0 != 0 && nm.uOldState & LVIS_SELECTED.0 == 0 {
                    let i = nm.iItem as usize;
                    if taken_h.get(i) == Some(&true) {
                        theme::set_text(error, "Ce port est déjà dans vos favoris");
                    } else if let Some((p, n)) = PRESETS.get(i) {
                        theme::set_text(port, &p.to_string());
                        theme::set_text(name, n);
                        theme::set_text(error, "");
                    }
                }
            }
            return None;
        }
        match command_id(msg, wp)? {
            CANCEL_ID => close(CANCEL_ID),
            OK_ID => match model::validate_favorite(
                &favs,
                editing_port,
                &theme::get_text(port),
                &theme::get_text(name),
                &theme::get_text(desc),
            ) {
                Ok(f) => {
                    *out_h.borrow_mut() = Some(f);
                    close(OK_ID);
                }
                Err(e) => theme::set_text(error, e),
            },
            _ => return None,
        }
        Some(LRESULT(0))
    });
    modal(
        owner,
        "OpenKiller",
        w,
        h,
        |b| {
            b.title(if library {
                "Ajouter un favori"
            } else {
                "Modifier le favori"
            });
            if library {
                let l = b.label(
                    "Bibliothèque : un clic préremplit le formulaire",
                    20,
                    48,
                    w - 40,
                    20,
                );
                theme::set_label_color(l, theme::DIM);
                let lv = theme::listview(b.hwnd, ID_PRESETS, false, true);
                send_lv_bg(lv);
                theme::lv_columns(lv, &[("Port", 90), ("Nom", 250), ("", 120)]);
                let rows: Vec<Vec<String>> = PRESETS
                    .iter()
                    .zip(&taken)
                    .map(|((p, n), t)| {
                        vec![
                            p.to_string(),
                            n.to_string(),
                            if *t {
                                "déjà en favori".into()
                            } else {
                                String::new()
                            },
                        ]
                    })
                    .collect();
                let params: Vec<isize> = (0..PRESETS.len() as isize).collect();
                theme::lv_fill(lv, &rows, &params);
                b.place(lv, 20, 72, w - 40, 308);
            }
            let f = editing.cloned().unwrap_or(Favorite {
                port: 0,
                name: String::new(),
                description: String::new(),
            });
            let port_txt = if f.port == 0 {
                String::new()
            } else {
                f.port.to_string()
            };
            let mut y = top;
            let row = |label: &str, y: i32| {
                let l = b.label(label, 20, y + 5, 110, 22);
                theme::set_label_color(l, theme::DIM);
            };
            row("Port *", y);
            let port = b.edit(
                ID_PORT,
                &port_txt,
                130,
                y,
                120,
                WINDOW_STYLE(ES_NUMBER as u32),
            );
            theme::send(port, EM_SETLIMITTEXT, 5, 0);
            y += 40;
            row("Nom *", y);
            let name = b.edit(ID_NAME, &f.name, 130, y, w - 150, WINDOW_STYLE(0));
            theme::send(name, EM_SETLIMITTEXT, model::NAME_MAX, 0);
            y += 40;
            row("Description", y);
            let desc = b.edit(ID_DESC, &f.description, 130, y, w - 150, WINDOW_STYLE(0));
            theme::send(desc, EM_SETLIMITTEXT, model::DESCRIPTION_MAX, 0);
            y += 40;
            let error = b.label("", 20, y + 4, w - 40, 22);
            theme::set_label_color(error, theme::DANGER);
            edits_b.set([port, name, desc, error]);
            b.buttons(
                w,
                h,
                if library { "Ajouter" } else { "Enregistrer" },
                Kind::Primary,
            );
            port
        },
        handler,
    );
    out.borrow_mut().take()
}

fn send_lv_bg(lv: HWND) {
    theme::send(lv, LVM_SETBKCOLOR, 0, theme::SURFACE.0 as isize);
    theme::send(lv, LVM_SETTEXTBKCOLOR, 0, theme::SURFACE.0 as isize);
}

/// Native "Save as" for one extension; None if cancelled.
pub fn save_file(owner: HWND, default_name: &str, ext: &str) -> Option<PathBuf> {
    unsafe {
        let dlg: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let (label, spec) = if ext == "csv" {
            (w!("CSV (*.csv)"), w!("*.csv"))
        } else {
            (w!("JSON (*.json)"), w!("*.json"))
        };
        dlg.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: label,
            pszSpec: spec,
        }])
        .ok()?;
        dlg.SetDefaultExtension(&HSTRING::from(ext)).ok()?;
        dlg.SetFileName(&HSTRING::from(default_name)).ok()?;
        dlg.Show(Some(owner)).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}
