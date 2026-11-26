//! Dark theme: colors, DPI-scaled fonts, owner-drawn buttons, themed controls and ListView helpers.

use std::cell::Cell;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR, w};

pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

pub const BG: COLORREF = rgb(0x1a, 0x1a, 0x1a);
pub const SIDE: COLORREF = rgb(0x12, 0x14, 0x17);
pub const SURFACE: COLORREF = rgb(0x22, 0x26, 0x2c);
pub const RAISED: COLORREF = rgb(0x2b, 0x31, 0x39);
pub const LINE: COLORREF = rgb(0x3a, 0x40, 0x48);
pub const TEXT: COLORREF = rgb(0xee, 0xf1, 0xf5);
pub const DIM: COLORREF = rgb(0x8b, 0x95, 0xa3);
pub const FAINT: COLORREF = rgb(0x5b, 0x64, 0x72);
pub const ACCENT: COLORREF = rgb(0xe8, 0x97, 0x3a);
pub const DANGER: COLORREF = rgb(0xef, 0x44, 0x59);
pub const WARN: COLORREF = rgb(0xf2, 0xc1, 0x4e);
pub const OK: COLORREF = rgb(0x2d, 0xd4, 0xbf);
pub const UDP: COLORREF = rgb(0x7c, 0x8f, 0xee);

// Static control styles (not exposed by the windows features we use).
pub const SS_CENTER: u32 = 0x1;
pub const SS_RIGHT: u32 = 0x2;
const SS_NOPREFIX: u32 = 0x80;
pub const SS_ENDELLIPSIS: u32 = 0x4000;

/// Owner-drawn button looks, stored in the button's GWLP_USERDATA.
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Normal = 1,
    Primary = 2,
    Danger = 3,
    Tab = 4,
    TabActive = 5,
}

struct Theme {
    dpi: Cell<u32>,
    font: Cell<HFONT>,
    bold: Cell<HFONT>,
    title: Cell<HFONT>,
    brushes: [HBRUSH; 3],
}

thread_local! {
    static THEME: Theme = Theme {
        dpi: Cell::new(96),
        font: Cell::new(HFONT::default()),
        bold: Cell::new(HFONT::default()),
        title: Cell::new(HFONT::default()),
        brushes: unsafe { [CreateSolidBrush(BG), CreateSolidBrush(SIDE), CreateSolidBrush(SURFACE)] },
    };
}

pub fn brush(c: COLORREF) -> HBRUSH {
    THEME.with(|t| match c {
        BG => t.brushes[0],
        SIDE => t.brushes[1],
        _ => t.brushes[2],
    })
}

pub fn dpi() -> u32 {
    THEME.with(|t| t.dpi.get())
}

/// DIP -> physical pixels at the current DPI.
pub fn px(dip: i32) -> i32 {
    dip * dpi() as i32 / 96
}

fn make_font(px_height: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -px_height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

/// (Re)creates the fonts for a DPI.
pub fn set_dpi(dpi: u32) {
    THEME.with(|t| {
        t.dpi.set(dpi);
        let old = [t.font.get(), t.bold.get(), t.title.get()];
        t.font.set(make_font(px(14), 400));
        t.bold.set(make_font(px(14), 600));
        t.title.set(make_font(px(22), 700));
        for f in old {
            if !f.is_invalid() {
                unsafe {
                    let _ = DeleteObject(f.into());
                }
            }
        }
    });
}

pub fn font() -> HFONT {
    THEME.with(|t| t.font.get())
}
pub fn bold() -> HFONT {
    THEME.with(|t| t.bold.get())
}
pub fn title_font() -> HFONT {
    THEME.with(|t| t.title.get())
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

pub fn set_text(hwnd: HWND, s: &str) {
    unsafe {
        let _ = SetWindowTextW(hwnd, &HSTRING::from(s));
    }
}

pub fn get_text(hwnd: HWND) -> String {
    unsafe {
        let mut buf = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

pub fn send(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wp)), Some(LPARAM(lp))).0 }
}

pub fn set_font(hwnd: HWND, font: HFONT) {
    send(hwnd, WM_SETFONT, font.0 as usize, 1);
}

/// Dark title bar (documented DWM attribute, Windows 10 20H1+ / 11).
pub fn dark_title(hwnd: HWND) {
    let on = BOOL(1);
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &on as *const _ as _, 4);
    }
}

pub fn child(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
    id: i32,
) -> HWND {
    unsafe {
        let h = CreateWindowExW(
            ex,
            class,
            &HSTRING::from(text),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as usize as _)),
            None,
            None,
        )
        .unwrap_or_default();
        set_font(h, font());
        h
    }
}

pub fn button(parent: HWND, id: i32, text: &str, kind: Kind) -> HWND {
    let h = child(
        parent,
        w!("BUTTON"),
        text,
        WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
        WINDOW_EX_STYLE(0),
        id,
    );
    set_kind(h, kind);
    h
}

pub fn set_kind(h: HWND, kind: Kind) {
    unsafe {
        SetWindowLongPtrW(h, GWLP_USERDATA, kind as isize);
        let _ = InvalidateRect(Some(h), None, true);
    }
}

/// Borderless edit: its parent paints a rounded frame around it (see paint_edit_frames).
pub fn edit(parent: HWND, id: i32, text: &str, extra: WINDOW_STYLE) -> HWND {
    child(
        parent,
        w!("EDIT"),
        text,
        WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32) | extra,
        WINDOW_EX_STYLE(0),
        id,
    )
}

/// Places an edit inside a frame rect (x, y, w, h in pixels), text vertically centered.
pub fn place_edit(h: HWND, x: i32, y: i32, w: i32, hh: i32, repaint: bool) {
    let th = px(20);
    unsafe {
        let _ = MoveWindow(
            h,
            x + px(8),
            y + (hh - th) / 2,
            (w - px(16)).max(0),
            th,
            repaint,
        );
    }
}

/// Rounded frames around the visible edits of `parent` (accent when focused).
pub fn paint_edit_frames(parent: HWND, hdc: HDC) {
    unsafe {
        let focus = windows::Win32::UI::Input::KeyboardAndMouse::GetFocus();
        let mut after = None;
        while let Ok(e) = FindWindowExW(Some(parent), after, w!("Edit"), None) {
            after = Some(e);
            if !IsWindowVisible(e).as_bool() {
                continue;
            }
            let mut r = RECT::default();
            let _ = GetWindowRect(e, &mut r);
            let _ = MapWindowPoints(
                None,
                Some(parent),
                &mut *(&mut r as *mut RECT as *mut [POINT; 2]),
            );
            let th = r.bottom - r.top;
            let frame = RECT {
                left: r.left - px(8),
                top: r.top - (px(30) - th) / 2,
                right: r.right + px(8),
                bottom: r.bottom + (px(30) - th + 1) / 2,
            };
            round_rect(
                hdc,
                &frame,
                SURFACE,
                if focus == e { ACCENT } else { LINE },
                px(8),
            );
        }
    }
}

pub fn label(parent: HWND, id: i32, text: &str, style: WINDOW_STYLE) -> HWND {
    child(
        parent,
        w!("STATIC"),
        text,
        style | WINDOW_STYLE(SS_NOPREFIX),
        WINDOW_EX_STYLE(0),
        id,
    )
}

/// WM_CTLCOLOR* for edits, list boxes and statics on a `bg` surface. Statics carrying a
/// COLORREF+1 in GWLP_USERDATA use it as text color (status/error colors).
pub fn ctl_color(msg: u32, wp: WPARAM, lp: LPARAM, bg: COLORREF) -> Option<LRESULT> {
    let hdc = HDC(wp.0 as _);
    unsafe {
        match msg {
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                SetTextColor(hdc, TEXT);
                SetBkColor(hdc, SURFACE);
                Some(LRESULT(brush(SURFACE).0 as isize))
            }
            WM_CTLCOLORSTATIC => {
                let custom = GetWindowLongPtrW(HWND(lp.0 as _), GWLP_USERDATA);
                SetTextColor(
                    hdc,
                    if custom > 0 {
                        COLORREF((custom - 1) as u32)
                    } else {
                        TEXT
                    },
                );
                SetBkColor(hdc, bg);
                Some(LRESULT(brush(bg).0 as isize))
            }
            _ => None,
        }
    }
}

pub fn set_label_color(h: HWND, c: COLORREF) {
    unsafe {
        SetWindowLongPtrW(h, GWLP_USERDATA, c.0 as isize + 1);
        let _ = InvalidateRect(Some(h), None, true);
    }
}

pub fn fill(hdc: HDC, r: &RECT, c: COLORREF) {
    unsafe {
        let b = CreateSolidBrush(c);
        FillRect(hdc, r, b);
        let _ = DeleteObject(b.into());
    }
}

pub fn round_rect(hdc: HDC, r: &RECT, fill_c: COLORREF, border: COLORREF, radius: i32) {
    unsafe {
        let b = CreateSolidBrush(fill_c);
        let p = CreatePen(PS_SOLID, 1, border);
        let ob = SelectObject(hdc, b.into());
        let op = SelectObject(hdc, p.into());
        let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        let _ = DeleteObject(b.into());
        let _ = DeleteObject(p.into());
    }
}

pub fn text(hdc: HDC, s: &str, r: &RECT, color: COLORREF, f: HFONT, flags: DRAW_TEXT_FORMAT) {
    let mut buf: Vec<u16> = s.encode_utf16().collect();
    let mut r = *r;
    unsafe {
        let of = SelectObject(hdc, f.into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color);
        DrawTextW(hdc, &mut buf, &mut r, flags | DT_NOPREFIX);
        SelectObject(hdc, of);
    }
}

/// WM_DRAWITEM for our owner-drawn buttons. `bg` = color behind the button.
pub fn draw_button(dis: &DRAWITEMSTRUCT) {
    if dis.CtlType != ODT_BUTTON {
        return;
    }
    let kind = unsafe { GetWindowLongPtrW(dis.hwndItem, GWLP_USERDATA) };
    let pressed = dis.itemState.0 & ODS_SELECTED.0 != 0;
    let disabled = dis.itemState.0 & ODS_DISABLED.0 != 0;
    let focus = dis.itemState.0 & ODS_FOCUS.0 != 0;
    let hdc = dis.hDC;
    let r = dis.rcItem;
    // Sidebar buttons have ids below 110; dialog buttons sit on SURFACE.
    let parent_bg = if !parent_is_main(dis.hwndItem) {
        SURFACE
    } else if dis.CtlID < 110 {
        SIDE
    } else {
        BG
    };
    fill(hdc, &r, parent_bg);
    let (fill_c, border, txt) = match kind {
        2 => (ACCENT, ACCENT, rgb(0x1a, 0x12, 0x04)),
        3 => (DANGER, DANGER, rgb(0xff, 0xff, 0xff)),
        4 => (parent_bg, parent_bg, DIM),
        5 => (RAISED, RAISED, TEXT),
        _ => (RAISED, LINE, TEXT),
    };
    let fill_c = if pressed {
        blend(fill_c, rgb(0, 0, 0), 30)
    } else {
        fill_c
    };
    round_rect(hdc, &r, fill_c, if focus { TEXT } else { border }, px(8));
    let txt = if disabled { FAINT } else { txt };
    let label = get_text(dis.hwndItem);
    if matches!(kind, 4 | 5) {
        if kind == 5 {
            let bar = RECT {
                left: r.left + px(1),
                top: r.top + px(7),
                right: r.left + px(4),
                bottom: r.bottom - px(7),
            };
            fill(hdc, &bar, ACCENT);
        }
        let tr = RECT {
            left: r.left + px(14),
            ..r
        };
        text(
            hdc,
            &label,
            &tr,
            txt,
            if kind == 5 { bold() } else { font() },
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    } else {
        text(
            hdc,
            &label,
            &r,
            txt,
            bold(),
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

fn parent_is_main(h: HWND) -> bool {
    unsafe {
        let mut buf = [0u16; 32];
        let n = GetClassNameW(GetParent(h).unwrap_or_default(), &mut buf);
        String::from_utf16_lossy(&buf[..n as usize]) == super::MAIN_CLASS
    }
}

/// Mix `a` towards `b` by pct %.
pub fn blend(a: COLORREF, b: COLORREF, pct: u32) -> COLORREF {
    let ch = |c: COLORREF, s: u32| (c.0 >> s) & 0xff;
    let mix = |s| (ch(a, s) * (100 - pct) + ch(b, s) * pct) / 100;
    COLORREF(mix(0) | mix(8) << 8 | mix(16) << 16)
}

// ---------- ListView ----------

pub fn listview(parent: HWND, id: i32, checkboxes: bool, single: bool) -> HWND {
    let mut style = WS_TABSTOP | WS_BORDER | WINDOW_STYLE(LVS_REPORT | LVS_SHOWSELALWAYS);
    if single {
        style |= WINDOW_STYLE(LVS_SINGLESEL);
    }
    let h = child(parent, WC_LISTVIEWW, "", style, WINDOW_EX_STYLE(0), id);
    let mut ex = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_INFOTIP;
    if checkboxes {
        ex |= LVS_EX_CHECKBOXES;
    }
    send(h, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as usize, ex as isize);
    send(h, LVM_SETBKCOLOR, 0, BG.0 as isize);
    send(h, LVM_SETTEXTBKCOLOR, 0, BG.0 as isize);
    send(h, LVM_SETTEXTCOLOR, 0, TEXT.0 as isize);
    unsafe {
        let _ = SetWindowTheme(h, w!("DarkMode_Explorer"), None);
        let header = HWND(send(h, LVM_GETHEADER, 0, 0) as _);
        let _ = SetWindowTheme(header, w!("DarkMode_ItemsView"), None);
        let _ = SetWindowSubclass(h, Some(lv_subclass), 1, 0);
    }
    h
}

/// Header items live inside the ListView: their custom draw arrives here, not at our window.
unsafe extern "system" fn lv_subclass(
    h: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if msg == WM_NOTIFY {
        let hdr = unsafe { &*(lp.0 as *const windows::Win32::UI::Controls::NMHDR) };
        if hdr.code == NM_CUSTOMDRAW && hdr.hwndFrom.0 as isize == send(h, LVM_GETHEADER, 0, 0) {
            let cd = unsafe { &*(lp.0 as *const NMCUSTOMDRAW) };
            match cd.dwDrawStage {
                CDDS_PREPAINT => return LRESULT(CDRF_NOTIFYITEMDRAW as isize),
                CDDS_ITEMPREPAINT => {
                    unsafe { SetTextColor(cd.hdc, DIM) };
                    return LRESULT(CDRF_DODEFAULT as isize);
                }
                _ => {}
            }
        }
    }
    unsafe { DefSubclassProc(h, msg, wp, lp) }
}

pub fn lv_columns(lv: HWND, cols: &[(&str, i32)]) {
    for (i, (name, width)) in cols.iter().enumerate() {
        let mut t = wide(name);
        let c = LVCOLUMNW {
            mask: LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
            cx: px(*width),
            pszText: PWSTR(t.as_mut_ptr()),
            iSubItem: i as i32,
            ..Default::default()
        };
        send(lv, LVM_INSERTCOLUMNW, i, &c as *const _ as isize);
    }
}

pub fn lv_rescale_columns(lv: HWND, cols: &[(&str, i32)]) {
    for (i, (_, width)) in cols.iter().enumerate() {
        send(lv, LVM_SETCOLUMNWIDTH, i, px(*width) as isize);
    }
}

/// Replaces all rows (cells as text, one lParam per row) without flicker.
pub fn lv_fill(lv: HWND, rows: &[Vec<String>], params: &[isize]) {
    send(lv, WM_SETREDRAW, 0, 0);
    send(lv, LVM_DELETEALLITEMS, 0, 0);
    for (i, row) in rows.iter().enumerate() {
        for (sub, cell) in row.iter().enumerate() {
            let mut t = wide(cell);
            let mut item = LVITEMW {
                iItem: i as i32,
                iSubItem: sub as i32,
                pszText: PWSTR(t.as_mut_ptr()),
                ..Default::default()
            };
            if sub == 0 {
                item.mask = LVIF_TEXT | LVIF_PARAM;
                item.lParam = LPARAM(params[i]);
                send(lv, LVM_INSERTITEMW, 0, &item as *const _ as isize);
            } else {
                item.mask = LVIF_TEXT;
                send(lv, LVM_SETITEMTEXTW, i, &item as *const _ as isize);
            }
        }
    }
    send(lv, WM_SETREDRAW, 1, 0);
    unsafe {
        let _ = InvalidateRect(Some(lv), None, true);
    }
}

pub fn lv_count(lv: HWND) -> usize {
    send(lv, LVM_GETITEMCOUNT, 0, 0) as usize
}

fn lv_state(lv: HWND, i: usize, mask: LIST_VIEW_ITEM_STATE_FLAGS) -> u32 {
    send(lv, LVM_GETITEMSTATE, i, mask.0 as isize) as u32
}

pub fn lv_is_selected(lv: HWND, i: usize) -> bool {
    lv_state(lv, i, LVIS_SELECTED) != 0
}

pub fn lv_is_checked(lv: HWND, i: usize) -> bool {
    lv_state(lv, i, LVIS_STATEIMAGEMASK) >> 12 == 2
}

pub fn lv_selected(lv: HWND) -> Option<usize> {
    let i = send(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize);
    (i >= 0).then_some(i as usize)
}

/// Sets selection/check state; `None` leaves that part unchanged. `i = usize::MAX` = all rows.
pub fn lv_set(lv: HWND, i: usize, selected: Option<bool>, checked: Option<bool>) {
    let mut item = LVITEMW::default();
    if let Some(s) = selected {
        item.stateMask.0 |= LVIS_SELECTED.0;
        if s {
            item.state.0 |= LVIS_SELECTED.0;
        }
    }
    if let Some(c) = checked {
        item.stateMask.0 |= LVIS_STATEIMAGEMASK.0;
        item.state.0 |= if c { 2 << 12 } else { 1 << 12 };
    }
    send(lv, LVM_SETITEMSTATE, i, &item as *const _ as isize);
}

pub fn lv_item_text(lv: HWND, i: usize, sub: i32) -> String {
    let mut buf = [0u16; 1024];
    let item = LVITEMW {
        iSubItem: sub,
        pszText: PWSTR(buf.as_mut_ptr()),
        cchTextMax: buf.len() as i32,
        ..Default::default()
    };
    let n = send(lv, LVM_GETITEMTEXTW, i, &item as *const _ as isize) as usize;
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

/// Sort arrow on one header column, none on the others.
pub fn lv_sort_arrow(lv: HWND, cols: usize, col: usize, desc: bool) {
    let header = HWND(send(lv, LVM_GETHEADER, 0, 0) as _);
    for i in 0..cols {
        let mut item = HDITEMW {
            mask: HDI_FORMAT,
            ..Default::default()
        };
        send(header, HDM_GETITEMW, i, &mut item as *mut _ as isize);
        item.fmt.0 &= !(HDF_SORTUP.0 | HDF_SORTDOWN.0);
        if i == col {
            item.fmt.0 |= if desc { HDF_SORTDOWN.0 } else { HDF_SORTUP.0 };
        }
        send(header, HDM_SETITEMW, i, &item as *const _ as isize);
    }
}

/// Row/sub-item under the mouse cursor.
pub fn lv_hit(lv: HWND) -> Option<(usize, i32)> {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
        let _ = windows::Win32::Graphics::Gdi::ScreenToClient(lv, &mut pt);
    }
    let mut hit = LVHITTESTINFO {
        pt,
        ..Default::default()
    };
    let i = send(lv, LVM_SUBITEMHITTEST, 0, &mut hit as *mut _ as isize);
    (i >= 0).then_some((i as usize, hit.iSubItem))
}

/// NM_CUSTOMDRAW for a ListView: `color(lParam, subitem)` picks the text color of a cell.
pub fn lv_custom_draw(lp: LPARAM, color: impl Fn(isize, i32) -> Option<COLORREF>) -> LRESULT {
    let cd = unsafe { &mut *(lp.0 as *mut NMLVCUSTOMDRAW) };
    match cd.nmcd.dwDrawStage {
        CDDS_PREPAINT => LRESULT(CDRF_NOTIFYITEMDRAW as isize),
        CDDS_ITEMPREPAINT => LRESULT(CDRF_NOTIFYSUBITEMDRAW as isize),
        s if s.0 == (CDDS_ITEMPREPAINT.0 | CDDS_SUBITEM.0) => {
            cd.clrText = color(cd.nmcd.lItemlParam.0, cd.iSubItem).unwrap_or(TEXT);
            LRESULT(CDRF_DODEFAULT as isize)
        }
        _ => LRESULT(CDRF_DODEFAULT as isize),
    }
}
