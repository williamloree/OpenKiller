//! Main window: sidebar (tabs, refresh, RAM gauge), toolbar, ports and favorites ListViews, status bar.

mod dialogs;
mod shot;
mod theme;

use crate::memory::{self, CleanError, MemInfo};
use crate::model::{self, Favorite, PortEntry, Proto, ProtoFilter, Sort, SortKey, fmt_gb};
use crate::{config, demo, export, ports, process, updater};
use dialogs::Target;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use theme::{Kind, px};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_F5, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

pub const MAIN_CLASS: &str = "OpenKillerMain";
pub const RESTART: i32 = 3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum View {
    Ports,
    Favorites,
    Kill,
    Presets,
}

impl View {
    /// `[]` or `["--view", name]`.
    pub fn parse(args: &[String]) -> Option<View> {
        match args {
            [] => Some(View::Ports),
            [flag, v] if flag == "--view" => match v.as_str() {
                "ports" => Some(View::Ports),
                "favorites" => Some(View::Favorites),
                "kill" => Some(View::Kill),
                "presets" => Some(View::Presets),
                _ => None,
            },
            _ => None,
        }
    }
}

pub enum Mode {
    Real,
    Demo,
    Shot(PathBuf, View),
}

// Control ids. Sidebar ids stay below 110 (theme::draw_button paints them on the sidebar color).
const ID_TAB_PORTS: i32 = 101;
const ID_TAB_FAVS: i32 = 102;
const ID_REFRESH: i32 = 103;
const ID_CLEAN: i32 = 104;
const ID_SEARCH: i32 = 110;
const ID_PROTO: i32 = 111;
const ID_CSV: i32 = 112;
const ID_JSON: i32 = 113;
const ID_KILL: i32 = 114;
const ID_FAV_ADD: i32 = 120;
const ID_FAV_EDIT: i32 = 121;
const ID_FAV_DEL: i32 = 122;
const ID_FAV_VIEW: i32 = 123;
const ID_FAV_KILL: i32 = 124;
const ID_LV_PORTS: i32 = 130;
const ID_LV_FAVS: i32 = 131;
// System menu entries (low 4 bits are reserved by Windows).
const IDM_AUTO_UPDATE: usize = 0x1010;
const IDM_CHECK_UPDATE: usize = 0x1020;

const TIMER_REFRESH: usize = 1;
const TIMER_SHOT: usize = 2;
const WM_APP_STATUS: u32 = WM_APP + 1;
const WM_APP_RESTART: u32 = WM_APP + 2;
const WM_APP_CLEANED: u32 = WM_APP + 3;

const PORT_COLS: [(&str, i32); 7] = [
    ("Port", 90),
    ("Protocole", 80),
    ("Adresse", 150),
    ("Application", 170),
    ("PID", 70),
    ("RAM (Mo)", 80),
    ("Chemin", 420),
];
const FAV_COLS: [(&str, i32); 5] = [
    ("Port", 90),
    ("Nom", 200),
    ("Description", 300),
    ("État", 90),
    ("Application", 220),
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Ports,
    Favorites,
}

struct Controls {
    tab_ports: HWND,
    tab_favs: HWND,
    refresh: HWND,
    clean: HWND,
    search: HWND,
    proto: HWND,
    csv: HWND,
    json: HWND,
    kill: HWND,
    fav: [HWND; 5],
    lv_ports: HWND,
    lv_favs: HWND,
    count: HWND,
    empty: HWND,
    status: HWND,
    scan: HWND,
}

struct App {
    hwnd: HWND,
    demo: bool,
    shot: Option<(PathBuf, View)>,
    c: Controls,
    tab: Tab,
    ports: Vec<PortEntry>,
    rows: Vec<PortEntry>,
    sort: Sort,
    filter: ProtoFilter,
    search: String,
    cfg: config::Config,
    mem: MemInfo,
    demo_killed: HashSet<u32>,
    clean_before: Option<u64>,
    exit_code: i32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the app state; None if not created yet or already borrowed (re-entrant
/// notification sent while we update a control, which we can safely ignore).
fn app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn status(hwnd: HWND, text: String) {
    // Posted, so it works even from inside a borrow or another thread.
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_APP_STATUS,
            WPARAM(0),
            LPARAM(Box::into_raw(Box::new(text)) as isize),
        );
    }
}

fn now() -> [u16; 6] {
    let t = unsafe { GetLocalTime() };
    [t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond]
}

pub fn focus_existing() {
    unsafe {
        if let Ok(h) = FindWindowW(&HSTRING::from(MAIN_CLASS), PCWSTR::null()) {
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
            }
            let _ = SetForegroundWindow(h);
        }
    }
}

fn register(class: &str, proc: WNDPROC, icon: HICON) {
    unsafe {
        let name = HSTRING::from(class);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: proc,
            hInstance: GetModuleHandleW(None).unwrap_or_default().into(),
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);
    }
}

pub fn run(mode: Mode) -> i32 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let icc = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let icon = LoadIconW(Some(hinst.into()), PCWSTR(1 as _)).unwrap_or_default();
        register(MAIN_CLASS, Some(wnd_proc), icon);
        register(dialogs::CLASS, Some(dialogs::proc), icon);

        let (demo, shot) = match &mode {
            Mode::Real => (false, None),
            Mode::Demo => (true, None),
            Mode::Shot(p, v) => (true, Some((p.clone(), *v))),
        };
        let mut cfg = if demo {
            config::Config::default()
        } else {
            config::load()
        };
        if demo {
            cfg.favorites = demo::favorites();
        }

        let style = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            &HSTRING::from(MAIN_CLASS),
            &HSTRING::from(if demo {
                "OpenKiller (démo)"
            } else {
                "OpenKiller"
            }),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinst.into()),
            None,
        ) else {
            return 1;
        };
        theme::set_dpi(GetDpiForWindow(hwnd));
        theme::dark_title(hwnd);
        place_window(hwnd, if shot.is_some() { None } else { cfg.window });

        let c = create_controls(hwnd);
        let menu = GetSystemMenu(hwnd, false);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let checked = if cfg.auto_update && !demo {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING | checked,
            IDM_AUTO_UPDATE,
            w!("Mises à jour automatiques"),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDM_CHECK_UPDATE,
            w!("Rechercher une mise à jour"),
        );

        let app_state = App {
            hwnd,
            demo,
            shot: shot.clone(),
            c,
            tab: Tab::Ports,
            ports: Vec::new(),
            rows: Vec::new(),
            sort: Sort::default(),
            filter: ProtoFilter::All,
            search: String::new(),
            cfg,
            mem: if demo {
                demo::memory()
            } else {
                MemInfo::default()
            },
            demo_killed: HashSet::new(),
            clean_before: None,
            exit_code: 0,
        };
        APP.with(|a| *a.borrow_mut() = Some(app_state));
        app(|a| {
            a.layout();
            a.refresh();
            a.show_tab(Tab::Ports);
            a.status(if demo {
                "Mode démo : données fictives, rien n'est modifié ni enregistré".into()
            } else {
                "Prêt".into()
            });
            if let Some((_, view)) = &a.shot {
                a.prepare_shot(*view);
            }
        });
        let _ = ShowWindow(hwnd, SW_SHOW);
        SetTimer(Some(hwnd), TIMER_REFRESH, 10_000, None);
        if shot.is_some() {
            SetTimer(Some(hwnd), TIMER_SHOT, 800, None);
        } else if !demo {
            start_auto_update(hwnd);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_KEYDOWN && handle_key(hwnd, msg.wParam.0 as u16) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        APP.with(|a| a.borrow_mut().take());
        msg.wParam.0 as i32
    }
}

/// Restores the saved rect if it is still on a monitor, else 1200x760 DIPs centered.
fn place_window(hwnd: HWND, saved: Option<[i32; 4]>) {
    unsafe {
        if let Some([x, y, w, h]) = saved {
            let r = RECT {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            };
            if !MonitorFromRect(&r, MONITOR_DEFAULTTONULL).is_invalid() {
                let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER);
                return;
            }
        }
        let (w, h) = (px(1200), px(760));
        let mut work = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = work.left + ((work.right - work.left - w) / 2).max(0);
        let y = work.top + ((work.bottom - work.top - h) / 2).max(0);
        let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER);
    }
}

fn create_controls(hwnd: HWND) -> Controls {
    let none = WINDOW_STYLE(0);
    let search = theme::edit(hwnd, ID_SEARCH, "", none);
    theme::send(
        search,
        EM_SETCUEBANNER,
        1,
        w!("Rechercher un port ou une application…").as_ptr() as isize,
    );
    let proto = theme::child(
        hwnd,
        w!("COMBOBOX"),
        "",
        WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        WINDOW_EX_STYLE(0),
        ID_PROTO,
    );
    unsafe {
        let _ = SetWindowTheme(proto, w!("DarkMode_CFD"), None);
    }
    for item in ["Tous protocoles", "TCP", "UDP"] {
        theme::send(
            proto,
            CB_ADDSTRING,
            0,
            HSTRING::from(item).as_ptr() as isize,
        );
    }
    theme::send(proto, CB_SETCURSEL, 0, 0);

    let lv_ports = theme::listview(hwnd, ID_LV_PORTS, true, false);
    theme::lv_columns(lv_ports, &PORT_COLS);
    theme::lv_sort_arrow(lv_ports, PORT_COLS.len(), 0, false);
    let lv_favs = theme::listview(hwnd, ID_LV_FAVS, false, true);
    theme::lv_columns(lv_favs, &FAV_COLS);

    let count = theme::label(hwnd, 0, "", none);
    theme::set_label_color(count, theme::DIM);
    let empty = theme::label(hwnd, 0, "", WINDOW_STYLE(theme::SS_CENTER));
    theme::set_label_color(empty, theme::DIM);
    let status = theme::label(hwnd, 0, "", WINDOW_STYLE(theme::SS_ENDELLIPSIS));
    let scan = theme::label(hwnd, 0, "", WINDOW_STYLE(theme::SS_RIGHT));
    theme::set_label_color(scan, theme::DIM);

    Controls {
        tab_ports: theme::button(hwnd, ID_TAB_PORTS, "Ports", Kind::TabActive),
        tab_favs: theme::button(hwnd, ID_TAB_FAVS, "Favoris", Kind::Tab),
        refresh: theme::button(hwnd, ID_REFRESH, "Actualiser (F5)", Kind::Normal),
        clean: theme::button(hwnd, ID_CLEAN, "Vider la RAM", Kind::Primary),
        search,
        proto,
        csv: theme::button(hwnd, ID_CSV, "Exporter CSV", Kind::Normal),
        json: theme::button(hwnd, ID_JSON, "Exporter JSON", Kind::Normal),
        kill: theme::button(hwnd, ID_KILL, "Tuer la sélection", Kind::Danger),
        fav: [
            theme::button(hwnd, ID_FAV_ADD, "Ajouter", Kind::Primary),
            theme::button(hwnd, ID_FAV_EDIT, "Modifier", Kind::Normal),
            theme::button(hwnd, ID_FAV_DEL, "Supprimer", Kind::Normal),
            theme::button(hwnd, ID_FAV_VIEW, "Voir", Kind::Normal),
            theme::button(hwnd, ID_FAV_KILL, "Tuer", Kind::Danger),
        ],
        lv_ports,
        lv_favs,
        count,
        empty,
        status,
        scan,
    }
}

/// Last column fills the remaining width (the full path stays available as a tooltip).
fn fit_last_column(lv: HWND, last: usize) {
    let mut cr = RECT::default();
    unsafe {
        let _ = GetClientRect(lv, &mut cr);
    }
    let others: isize = (0..last)
        .map(|i| theme::send(lv, LVM_GETCOLUMNWIDTH, i, 0))
        .sum();
    theme::send(
        lv,
        LVM_SETCOLUMNWIDTH,
        last,
        (cr.right as isize - others).max(px(120) as isize),
    );
}

fn move_to(h: HWND, x: i32, y: i32, w: i32, hh: i32) {
    unsafe {
        let _ = MoveWindow(h, x, y, w.max(0), hh.max(0), true);
    }
}

fn show(h: HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(h, if visible { SW_SHOW } else { SW_HIDE });
    }
}

const SIDEBAR: i32 = 200;

/// Gauge area at the bottom of the sidebar (above the "Vider la RAM" button).
fn gauge_rect(client: &RECT) -> RECT {
    RECT {
        left: px(16),
        top: client.bottom - px(150),
        right: px(SIDEBAR - 16),
        bottom: client.bottom - px(64),
    }
}

impl App {
    fn client(&self) -> RECT {
        let mut r = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut r);
        }
        r
    }

    fn layout(&self) {
        let r = self.client();
        let c = &self.c;
        let (pad, gap) = (px(16), px(8));
        let side_w = px(SIDEBAR) - 2 * pad;
        move_to(c.tab_ports, pad, px(84), side_w, px(36));
        move_to(c.tab_favs, pad, px(124), side_w, px(36));
        move_to(c.refresh, pad, px(176), side_w, px(32));
        move_to(c.clean, pad, r.bottom - px(52), side_w, px(34));

        let x0 = px(SIDEBAR) + pad;
        let right = r.right - pad;
        // The combo's height comes from its font: align the whole toolbar on it.
        move_to(c.proto, 0, pad, px(140), px(200));
        let mut cr = RECT::default();
        unsafe {
            let _ = GetWindowRect(c.proto, &mut cr);
        }
        let row_h = (cr.bottom - cr.top).max(px(28));
        let mut x = right;
        for (h, w) in [(c.kill, 150), (c.json, 120), (c.csv, 120)] {
            x -= px(w);
            move_to(h, x, pad, px(w), row_h);
            x -= gap;
        }
        x -= px(140);
        move_to(c.proto, x, pad, px(140), px(200));
        theme::place_edit(c.search, x0, pad, x - gap - x0, row_h, true);
        let mut fx = x0;
        for h in c.fav {
            move_to(h, fx, pad, px(104), row_h);
            fx += px(104) + gap;
        }
        let list_top = pad + row_h + px(34);
        move_to(c.count, x0, pad + row_h + px(10), right - x0, px(20));
        let list_bottom = r.bottom - px(34);
        for (lv, last) in [
            (c.lv_ports, PORT_COLS.len() - 1),
            (c.lv_favs, FAV_COLS.len() - 1),
        ] {
            move_to(lv, x0, list_top, right - x0, list_bottom - list_top);
            fit_last_column(lv, last);
        }
        move_to(c.empty, x0, list_top + px(60), right - x0, px(24));
        let status_w = (right - x0) * 3 / 5;
        move_to(c.status, x0, r.bottom - px(26), status_w, px(20));
        move_to(
            c.scan,
            x0 + status_w,
            r.bottom - px(26),
            right - x0 - status_w,
            px(20),
        );
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn status(&self, text: String) {
        theme::set_text(self.c.status, &text);
    }

    fn refresh(&mut self) {
        let t = Instant::now();
        self.ports = if self.demo {
            demo::ports()
                .into_iter()
                .filter(|p| !self.demo_killed.contains(&p.pid))
                .collect()
        } else {
            ports::scan()
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        #[cfg(debug_assertions)]
        eprintln!("scan: {} rows in {ms:.1} ms", self.ports.len());
        if !self.demo {
            self.mem = memory::read();
        }
        let n = now();
        theme::set_text(
            self.c.scan,
            &format!(
                "Dernier scan : {:02}:{:02}:{:02} · {ms:.0} ms",
                n[3], n[4], n[5]
            ),
        );
        self.render_ports(false);
        self.render_favorites();
        self.invalidate_gauge();
    }

    fn invalidate_gauge(&self) {
        let r = gauge_rect(&self.client());
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), Some(&r), false);
        }
    }

    /// Rebuilds the ports list only if the visible rows changed, keeping selection, checks and scroll.
    fn render_ports(&mut self, force: bool) {
        let rows = model::visible(&self.ports, &self.search, self.filter, self.sort);
        let lv = self.c.lv_ports;
        if force || rows != self.rows {
            type Key = (Proto, u16, u32);
            let key = |p: &PortEntry| -> Key { (p.proto, p.port, p.pid) };
            let n = theme::lv_count(lv);
            let selected: HashSet<Key> = (0..n.min(self.rows.len()))
                .filter(|&i| theme::lv_is_selected(lv, i))
                .map(|i| key(&self.rows[i]))
                .collect();
            let checked: HashSet<Key> = (0..n.min(self.rows.len()))
                .filter(|&i| theme::lv_is_checked(lv, i))
                .map(|i| key(&self.rows[i]))
                .collect();
            let top = theme::send(lv, LVM_GETTOPINDEX, 0, 0).max(0);
            let cells: Vec<Vec<String>> = rows
                .iter()
                .map(|p| {
                    vec![
                        p.port.to_string(),
                        p.proto.as_str().into(),
                        p.address.clone(),
                        p.name.clone(),
                        p.pid.to_string(),
                        p.memory_mb.map_or("N/A".into(), |m| m.to_string()),
                        p.path.clone().unwrap_or_else(|| "N/A".into()),
                    ]
                })
                .collect();
            let params: Vec<isize> = rows
                .iter()
                .map(|p| (p.proto == Proto::Udp) as isize)
                .collect();
            theme::lv_fill(lv, &cells, &params);
            fit_last_column(lv, PORT_COLS.len() - 1);
            for (i, p) in rows.iter().enumerate() {
                let (s, c) = (selected.contains(&key(p)), checked.contains(&key(p)));
                if s || c {
                    theme::lv_set(lv, i, Some(s), Some(c));
                }
            }
            if top > 0 && !rows.is_empty() {
                let mut ir = RECT {
                    left: LVIR_BOUNDS as i32,
                    ..Default::default()
                };
                theme::send(lv, LVM_GETITEMRECT, 0, &mut ir as *mut _ as isize);
                theme::send(lv, LVM_SCROLL, 0, top * (ir.bottom - ir.top) as isize);
            }
            self.rows = rows;
        }
        let label = model::count_label(self.rows.len(), self.ports.len());
        theme::set_text(self.c.count, &label);
        theme::set_text(self.c.tab_ports, &format!("Ports ({})", self.ports.len()));
        self.update_empty();
    }

    fn render_favorites(&mut self) {
        let cells: Vec<Vec<String>> = self
            .cfg
            .favorites
            .iter()
            .map(|f| {
                let mut apps: Vec<&str> = self
                    .ports
                    .iter()
                    .filter(|p| p.port == f.port)
                    .map(|p| p.name.as_str())
                    .collect();
                apps.dedup();
                vec![
                    f.port.to_string(),
                    f.name.clone(),
                    f.description.clone(),
                    if apps.is_empty() {
                        "Inactif".into()
                    } else {
                        "● Actif".into()
                    },
                    apps.join(", "),
                ]
            })
            .collect();
        let params: Vec<isize> = cells.iter().map(|c| (!c[4].is_empty()) as isize).collect();
        let lv = self.c.lv_favs;
        let sel = theme::lv_selected(lv);
        theme::lv_fill(lv, &cells, &params);
        fit_last_column(lv, FAV_COLS.len() - 1);
        if let Some(i) = sel.filter(|&i| i < cells.len()) {
            theme::lv_set(lv, i, Some(true), None);
        }
        theme::set_text(self.c.tab_favs, &format!("Favoris ({})", cells.len()));
        if self.tab == Tab::Favorites {
            theme::set_text(self.c.count, &model::favorites_label(cells.len()));
        }
        self.update_empty();
    }

    fn update_empty(&self) {
        let (lv, n, text) = match self.tab {
            Tab::Ports => (self.c.lv_ports, self.rows.len(), "Aucun port trouvé"),
            Tab::Favorites => (
                self.c.lv_favs,
                self.cfg.favorites.len(),
                "Aucun favori – ajoute un port depuis la bibliothèque",
            ),
        };
        theme::set_text(self.c.empty, text);
        show(lv, n > 0);
        show(self.c.empty, n == 0);
    }

    fn show_tab(&mut self, tab: Tab) {
        self.tab = tab;
        let p = tab == Tab::Ports;
        theme::set_kind(
            self.c.tab_ports,
            if p { Kind::TabActive } else { Kind::Tab },
        );
        theme::set_kind(self.c.tab_favs, if p { Kind::Tab } else { Kind::TabActive });
        for h in [
            self.c.search,
            self.c.proto,
            self.c.csv,
            self.c.json,
            self.c.kill,
        ] {
            show(h, p);
        }
        for h in self.c.fav {
            show(h, !p);
        }
        show(self.c.lv_ports, false);
        show(self.c.lv_favs, false);
        if p {
            self.render_ports(false);
        } else {
            self.render_favorites();
        }
    }

    fn save_config(&self) {
        if !self.demo
            && let Err(e) = config::save(&self.cfg)
        {
            self.status(format!("Impossible d'enregistrer la configuration : {e}"));
        }
    }

    /// Checked rows, or the selected ones when nothing is checked.
    fn port_targets(&self) -> Vec<Target> {
        let lv = self.c.lv_ports;
        let n = theme::lv_count(lv).min(self.rows.len());
        let checked: Vec<&PortEntry> = (0..n)
            .filter(|&i| theme::lv_is_checked(lv, i))
            .map(|i| &self.rows[i])
            .collect();
        let picked = if checked.is_empty() {
            (0..n)
                .filter(|&i| theme::lv_is_selected(lv, i))
                .map(|i| &self.rows[i])
                .collect()
        } else {
            checked
        };
        model::distinct_pids(&picked)
    }

    fn selected_favorite(&self) -> Option<Favorite> {
        theme::lv_selected(self.c.lv_favs).and_then(|i| self.cfg.favorites.get(i).cloned())
    }

    fn favorite_targets(&self, f: &Favorite) -> Vec<Target> {
        let rows: Vec<&PortEntry> = self.ports.iter().filter(|p| p.port == f.port).collect();
        model::distinct_pids(&rows)
    }

    fn kill(&mut self, targets: &[Target]) {
        let mut ok = 0;
        let mut errors: Vec<String> = Vec::new();
        for (pid, name, _) in targets {
            let r = if self.demo {
                if model::is_protected(*pid, std::process::id()) {
                    Err(process::KillError::Protected)
                } else {
                    self.demo_killed.insert(*pid);
                    Ok(())
                }
            } else {
                process::kill(*pid, name)
            };
            match r {
                Ok(()) => ok += 1,
                Err(e) => errors.push(format!("PID {pid} : {}", e.message())),
            }
        }
        let suffix = if self.demo { " (démo)" } else { "" };
        let msg = match (targets, ok) {
            ([(pid, name, _)], 1) => format!("Processus {name} (PID {pid}) terminé{suffix}"),
            ([_], _) => errors.join(""),
            (_, _) if errors.is_empty() => format!("{ok} processus terminés{suffix}"),
            _ => format!(
                "{ok} processus terminé(s), {} échec(s) : {}",
                errors.len(),
                errors.join(" ; ")
            ),
        };
        self.status(msg);
        self.refresh();
    }

    fn export(&mut self, csv: bool) {
        if self.rows.is_empty() {
            self.status("Aucune donnée à exporter".into());
            return;
        }
        let ext = if csv { "csv" } else { "json" };
        let name = export::file_name(now(), ext);
        let Some(path) = dialogs::save_file(self.hwnd, &name, ext) else {
            return;
        };
        if self.demo {
            self.status(format!(
                "Export {} simulé (démo) : rien n'a été écrit",
                ext.to_uppercase()
            ));
            return;
        }
        let data = if csv {
            export::csv(&self.rows)
        } else {
            export::json(&self.rows).into_bytes()
        };
        self.status(match std::fs::write(&path, data) {
            Ok(()) => format!("Export {} réussi : {}", ext.to_uppercase(), path.display()),
            Err(e) => format!("Échec de l'export : {e}"),
        });
    }

    fn clear_search(&mut self) {
        self.search.clear();
        theme::set_text(self.c.search, "");
        self.render_ports(false);
    }

    fn select_all(&self) {
        let lv = if self.tab == Tab::Ports {
            self.c.lv_ports
        } else {
            self.c.lv_favs
        };
        theme::lv_set(lv, usize::MAX, Some(true), None);
    }

    fn sort_click(&mut self, col: i32) {
        let key = match col {
            0 => SortKey::Port,
            3 => SortKey::App,
            _ => return,
        };
        self.sort.click(key);
        theme::lv_sort_arrow(
            self.c.lv_ports,
            PORT_COLS.len(),
            col as usize,
            self.sort.desc,
        );
        self.render_ports(false);
    }

    fn view_favorite(&mut self) {
        if let Some(f) = self.selected_favorite() {
            self.show_tab(Tab::Ports);
            self.search = f.port.to_string();
            theme::set_text(self.c.search, &self.search);
            self.render_ports(false);
        }
    }

    fn rescale(&self) {
        for h in [
            self.c.tab_ports,
            self.c.tab_favs,
            self.c.refresh,
            self.c.clean,
            self.c.search,
            self.c.proto,
            self.c.csv,
            self.c.json,
            self.c.kill,
            self.c.lv_ports,
            self.c.lv_favs,
            self.c.count,
            self.c.empty,
            self.c.status,
            self.c.scan,
        ]
        .into_iter()
        .chain(self.c.fav)
        {
            theme::set_font(h, theme::font());
        }
        theme::lv_rescale_columns(self.c.lv_ports, &PORT_COLS);
        theme::lv_rescale_columns(self.c.lv_favs, &FAV_COLS);
    }

    fn prepare_shot(&mut self, view: View) {
        match view {
            View::Favorites | View::Presets => self.show_tab(Tab::Favorites),
            View::Kill => {
                // 3 rows of 2 PIDs: caddy 80 + 443, node 3000.
                for (i, p) in self.rows.iter().enumerate() {
                    if matches!(p.port, 80 | 443 | 3000) {
                        theme::lv_set(self.c.lv_ports, i, None, Some(true));
                    }
                }
            }
            View::Ports => {}
        }
        if view == View::Favorites {
            theme::lv_set(self.c.lv_favs, 0, Some(true), None);
        }
    }

    fn save_window(&mut self) {
        let mut wp = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetWindowPlacement(self.hwnd, &mut wp).is_ok() {
                let r = wp.rcNormalPosition;
                self.cfg.window = Some([r.left, r.top, r.right - r.left, r.bottom - r.top]);
            }
        }
        self.save_config();
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut r = RECT::default();
        let _ = GetClientRect(hwnd, &mut r);
        let side = RECT {
            right: px(SIDEBAR),
            ..r
        };
        theme::fill(
            hdc,
            &RECT {
                left: side.right,
                ..r
            },
            theme::BG,
        );
        theme::fill(hdc, &side, theme::SIDE);
        theme::fill(
            hdc,
            &RECT {
                left: side.right - 1,
                ..side
            },
            theme::rgb(0x26, 0x2b, 0x32),
        );
        let flags = DT_LEFT | DT_SINGLELINE | DT_VCENTER;
        theme::paint_edit_frames(hwnd, hdc);
        theme::text(
            hdc,
            "OpenKiller",
            &RECT {
                left: px(18),
                top: px(18),
                right: side.right,
                bottom: px(48),
            },
            theme::TEXT,
            theme::title_font(),
            flags,
        );
        let sub = format!("Ports & processus · v{}", updater::VERSION);
        theme::text(
            hdc,
            &sub,
            &RECT {
                left: px(18),
                top: px(48),
                right: side.right,
                bottom: px(68),
            },
            theme::DIM,
            theme::font(),
            flags,
        );

        let mem = app(|a| a.mem).unwrap_or_default();
        let g = gauge_rect(&r);
        theme::fill(hdc, &g, theme::SIDE);
        let line = |top: i32, h: i32| RECT {
            left: g.left,
            top: g.top + px(top),
            right: g.right,
            bottom: g.top + px(top + h),
        };
        theme::text(hdc, "RAM", &line(0, 20), theme::DIM, theme::bold(), flags);
        if mem.total > 0 {
            let used = format!(
                "{} / {}",
                fmt_gb(mem.used).trim_end_matches(" Go"),
                fmt_gb(mem.total)
            );
            theme::text(
                hdc,
                &used,
                &line(0, 20),
                theme::TEXT,
                theme::bold(),
                DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
            );
            let bar = line(28, 10);
            theme::round_rect(hdc, &bar, theme::SURFACE, theme::LINE, px(6));
            let width = (bar.right - bar.left) as f64;
            let used_w = (width * mem.used as f64 / mem.total as f64) as i32;
            let free_w = (width * mem.freeable as f64 / mem.total as f64) as i32;
            if used_w > 0 {
                let u = RECT {
                    right: bar.left + used_w,
                    ..bar
                };
                theme::round_rect(hdc, &u, theme::ACCENT, theme::ACCENT, px(6));
                let f = RECT {
                    left: (u.right - free_w).max(bar.left),
                    right: u.right,
                    ..bar
                };
                theme::round_rect(hdc, &f, theme::WARN, theme::WARN, px(6));
            }
            theme::text(
                hdc,
                &format!("Libérable : {}", fmt_gb(mem.freeable)),
                &line(46, 20),
                theme::DIM,
                theme::font(),
                flags,
            );
        } else {
            theme::text(
                hdc,
                "indisponible",
                &line(0, 20),
                theme::DIM,
                theme::font(),
                DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
            );
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

/// Keyboard shortcuts of the main window. Returns true when the key was consumed.
fn handle_key(hwnd: HWND, key: u16) -> bool {
    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
    let focus = unsafe { GetFocus() };
    let Some((search, in_main)) = app(|a| {
        (a.c.search, unsafe {
            focus == hwnd || IsChild(hwnd, focus).as_bool()
        })
    }) else {
        return false;
    };
    if !in_main {
        return false;
    }
    let cmd = |id: i32| unsafe {
        let _ = PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(id as usize), LPARAM(0));
    };
    match key {
        k if k == VK_F5.0 => cmd(ID_REFRESH),
        k if k == VK_ESCAPE.0 => {
            app(|a| a.clear_search());
        }
        k if k == u16::from(b'F') && ctrl => {
            app(|a| a.show_tab(Tab::Ports));
            unsafe {
                let _ = SetFocus(Some(search));
            }
            theme::send(search, EM_SETSEL, 0, -1);
        }
        k if k == u16::from(b'A') && ctrl => {
            if focus == search {
                theme::send(search, EM_SETSEL, 0, -1);
            } else {
                app(|a| a.select_all());
            }
        }
        k if k == VK_DELETE.0 && focus != search => {
            let tab = app(|a| a.tab);
            cmd(if tab == Some(Tab::Favorites) {
                ID_FAV_KILL
            } else {
                ID_KILL
            });
        }
        k if k == VK_RETURN.0 && dialogs::is_button(focus) => {
            theme::send(focus, BM_CLICK, 0, 0);
        }
        _ => return false,
    }
    true
}

fn start_auto_update(hwnd: HWND) {
    let h = hwnd.0 as isize;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        loop {
            if config::load().auto_update
                && let Ok(updater::Outcome::Installed(tag)) = updater::check_and_install(false)
            {
                let hwnd = HWND(h as _);
                status(hwnd, format!("Mise à jour {tag} installée, redémarrage…"));
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_APP_RESTART, WPARAM(0), LPARAM(0));
                }
                return;
            }
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

fn check_update_now(hwnd: HWND) {
    status(hwnd, "Recherche d'une mise à jour…".into());
    let h = hwnd.0 as isize;
    std::thread::spawn(move || {
        let hwnd = HWND(h as _);
        match updater::check_and_install(true) {
            Ok(updater::Outcome::UpToDate) => status(
                hwnd,
                format!("OpenKiller est à jour (v{})", updater::VERSION),
            ),
            Ok(updater::Outcome::Installed(tag)) => {
                status(hwnd, format!("Mise à jour {tag} installée, redémarrage…"));
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_APP_RESTART, WPARAM(0), LPARAM(0));
                }
            }
            Err(e) => status(hwnd, format!("Mise à jour impossible : {e}")),
        }
    });
}

/// WM_COMMAND actions that may open a modal dialog run here, outside any APP borrow.
fn command(hwnd: HWND, id: i32, code: u32) {
    match id {
        ID_TAB_PORTS => {
            app(|a| a.show_tab(Tab::Ports));
        }
        ID_TAB_FAVS => {
            app(|a| a.show_tab(Tab::Favorites));
        }
        ID_REFRESH => {
            app(|a| {
                a.refresh();
                a.status("Liste actualisée".into());
            });
        }
        ID_SEARCH if code == EN_SETFOCUS || code == EN_KILLFOCUS => unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        },
        ID_SEARCH if code == EN_CHANGE => {
            app(|a| {
                a.search = theme::get_text(a.c.search);
                a.render_ports(false);
            });
        }
        ID_PROTO if code == CBN_SELCHANGE => {
            app(|a| {
                a.filter = match theme::send(a.c.proto, CB_GETCURSEL, 0, 0) {
                    1 => ProtoFilter::Tcp,
                    2 => ProtoFilter::Udp,
                    _ => ProtoFilter::All,
                };
                a.render_ports(false);
            });
        }
        ID_CSV | ID_JSON => {
            app(|a| a.export(id == ID_CSV));
        }
        ID_KILL | ID_FAV_KILL => {
            let targets = if id == ID_KILL {
                app(|a| Ok(a.port_targets()))
            } else {
                app(|a| match a.selected_favorite() {
                    None => Err("Sélectionne un favori".to_string()),
                    Some(f) => {
                        let t = a.favorite_targets(&f);
                        if t.is_empty() {
                            Err(format!("Port {} inactif : rien à tuer", f.port))
                        } else {
                            Ok(t)
                        }
                    }
                })
            };
            match targets {
                Some(Ok(t)) if t.is_empty() => {
                    status(hwnd, "Coche ou sélectionne au moins une ligne".into())
                }
                Some(Ok(t)) => {
                    if dialogs::confirm_kill(hwnd, &t) {
                        app(|a| a.kill(&t));
                    }
                }
                Some(Err(e)) => status(hwnd, e),
                None => {}
            }
        }
        ID_FAV_ADD | ID_FAV_EDIT => {
            let Some((favs, sel)) = app(|a| (a.cfg.favorites.clone(), a.selected_favorite()))
            else {
                return;
            };
            if id == ID_FAV_EDIT && sel.is_none() {
                status(hwnd, "Sélectionne un favori à modifier".into());
                return;
            }
            let editing = if id == ID_FAV_EDIT {
                sel.as_ref()
            } else {
                None
            };
            if let Some(f) = dialogs::favorite(hwnd, &favs, editing) {
                app(|a| {
                    let msg = match editing
                        .and_then(|e| a.cfg.favorites.iter().position(|x| x.port == e.port))
                    {
                        Some(i) => {
                            a.cfg.favorites[i] = f.clone();
                            format!("Favori port {} mis à jour", f.port)
                        }
                        None => {
                            a.cfg.favorites.push(f.clone());
                            format!("Port {} ajouté aux favoris", f.port)
                        }
                    };
                    a.save_config();
                    a.render_favorites();
                    a.status(msg);
                });
            }
        }
        ID_FAV_DEL => {
            let Some(Some(f)) = app(|a| a.selected_favorite()) else {
                status(hwnd, "Sélectionne un favori à supprimer".into());
                return;
            };
            let msg = format!("Retirer le port {} ({}) des favoris ?", f.port, f.name);
            if dialogs::confirm(hwnd, "Supprimer le favori", &msg, "Supprimer", Kind::Danger) {
                app(|a| {
                    a.cfg.favorites.retain(|x| x.port != f.port);
                    a.save_config();
                    a.render_favorites();
                    a.status(format!("Port {} retiré des favoris", f.port));
                });
            }
        }
        ID_FAV_VIEW => {
            app(|a| a.view_favorite());
        }
        ID_CLEAN => {
            let Some((mem, demo, busy)) = app(|a| (a.mem, a.demo, a.clean_before.is_some())) else {
                return;
            };
            if busy || !dialogs::confirm_clean(hwnd, &mem) {
                return;
            }
            if demo {
                app(|a| {
                    let freed = a.mem.freeable * 4 / 5;
                    a.mem.used -= freed;
                    a.mem.freeable -= freed;
                    a.invalidate_gauge();
                    a.status(format!("RAM libérée : {} (démo)", fmt_gb(freed)));
                });
                return;
            }
            app(|a| {
                a.clean_before = Some(memory::read().used);
                unsafe {
                    let _ = EnableWindow(a.c.clean, false);
                }
                a.status("Vidage de la RAM en cours…".into());
            });
            let h = hwnd.0 as isize;
            std::thread::spawn(move || {
                let hwnd = HWND(h as _);
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                }
                let (code, err) = match memory::run_elevated_clean(hwnd) {
                    Ok(()) => (0, String::new()),
                    Err(CleanError::Cancelled) => (1, String::new()),
                    Err(CleanError::Failed(e)) => (2, e),
                };
                unsafe {
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_APP_CLEANED,
                        WPARAM(code),
                        LPARAM(Box::into_raw(Box::new(err)) as isize),
                    );
                }
            });
        }
        _ => {}
    }
}

fn take_shot(hwnd: HWND) {
    let Some(Some((path, view))) = app(|a| a.shot.clone()) else {
        return;
    };
    let ok = match view {
        View::Ports | View::Favorites => shot::capture(hwnd, None, &path).is_ok(),
        View::Kill | View::Presets => {
            dialogs::SHOT.with(|s| *s.borrow_mut() = Some((path, hwnd)));
            dialogs::SHOT_OK.with(|o| o.set(false));
            command(
                hwnd,
                if view == View::Kill {
                    ID_KILL
                } else {
                    ID_FAV_ADD
                },
                0,
            );
            dialogs::SHOT_OK.with(|o| o.get())
        }
    };
    unsafe { PostQuitMessage(if ok { 0 } else { 1 }) };
}

use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let Some(r) = theme::ctl_color(msg, wp, lp, theme::BG) {
        return r;
    }
    match msg {
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            paint(hwnd);
            return LRESULT(0);
        }
        WM_DRAWITEM => {
            theme::draw_button(unsafe { &*(lp.0 as *const DRAWITEMSTRUCT) });
            return LRESULT(1);
        }
        WM_SIZE => {
            app(|a| a.layout());
        }
        WM_GETMINMAXINFO => {
            let mmi = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
            mmi.ptMinTrackSize = POINT {
                x: 900 * dpi / 96,
                y: 560 * dpi / 96,
            };
            return LRESULT(0);
        }
        WM_DPICHANGED => {
            theme::set_dpi((wp.0 & 0xffff) as u32);
            let r = unsafe { &*(lp.0 as *const RECT) };
            app(|a| a.rescale());
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            app(|a| a.layout());
            return LRESULT(0);
        }
        WM_TIMER => match wp.0 {
            TIMER_REFRESH => {
                app(|a| a.refresh());
            }
            TIMER_SHOT => {
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_SHOT);
                }
                take_shot(hwnd);
            }
            _ => {}
        },
        WM_COMMAND => {
            command(hwnd, (wp.0 & 0xffff) as i32, ((wp.0 >> 16) & 0xffff) as u32);
            return LRESULT(0);
        }
        WM_NOTIFY => {
            let hdr = unsafe { &*(lp.0 as *const NMHDR) };
            let id = hdr.idFrom as i32;
            match hdr.code {
                NM_CUSTOMDRAW if id == ID_LV_PORTS => {
                    return theme::lv_custom_draw(lp, |udp, sub| {
                        (sub == 1).then_some(if udp == 1 { theme::UDP } else { theme::OK })
                    });
                }
                NM_CUSTOMDRAW if id == ID_LV_FAVS => {
                    return theme::lv_custom_draw(lp, |active, sub| {
                        (sub == 3).then_some(if active == 1 { theme::OK } else { theme::FAINT })
                    });
                }
                LVN_COLUMNCLICK if id == ID_LV_PORTS => {
                    let nm = unsafe { &*(lp.0 as *const NMLISTVIEW) };
                    app(|a| a.sort_click(nm.iSubItem));
                }
                NM_DBLCLK if id == ID_LV_FAVS => command(hwnd, ID_FAV_EDIT, 0),
                LVN_GETINFOTIPW if id == ID_LV_PORTS => {
                    // Full path tooltip when hovering the "Chemin" column.
                    let tip = unsafe { &mut *(lp.0 as *mut NMLVGETINFOTIPW) };
                    if let Some((row, 6)) = theme::lv_hit(hdr.hwndFrom) {
                        let path = theme::lv_item_text(hdr.hwndFrom, row, 6);
                        let n = (tip.cchTextMax as usize).saturating_sub(1);
                        let src: Vec<u16> = path.encode_utf16().take(n).chain([0]).collect();
                        unsafe {
                            std::ptr::copy_nonoverlapping(src.as_ptr(), tip.pszText.0, src.len())
                        };
                    } else if tip.cchTextMax > 0 {
                        unsafe { *tip.pszText.0 = 0 };
                    }
                }
                _ => {}
            }
        }
        WM_SYSCOMMAND if wp.0 & 0xfff0 == IDM_CHECK_UPDATE => {
            if app(|a| a.demo) == Some(true) {
                status(hwnd, "Mises à jour désactivées en mode démo".into());
            } else {
                check_update_now(hwnd);
            }
            return LRESULT(0);
        }
        WM_SYSCOMMAND if wp.0 & 0xfff0 == IDM_AUTO_UPDATE => {
            app(|a| {
                if a.demo {
                    a.status("Mises à jour désactivées en mode démo".into());
                    return;
                }
                a.cfg.auto_update = !a.cfg.auto_update;
                a.save_config();
                unsafe {
                    let flag = if a.cfg.auto_update {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    };
                    CheckMenuItem(
                        GetSystemMenu(hwnd, false),
                        IDM_AUTO_UPDATE as u32,
                        (MF_BYCOMMAND | flag).0,
                    );
                }
                a.status(format!(
                    "Mises à jour automatiques {}",
                    if a.cfg.auto_update {
                        "activées"
                    } else {
                        "désactivées"
                    }
                ));
            });
            return LRESULT(0);
        }
        WM_APP_STATUS => {
            let text = unsafe { Box::from_raw(lp.0 as *mut String) };
            app(|a| a.status(*text));
            return LRESULT(0);
        }
        WM_APP_CLEANED => {
            let err = unsafe { Box::from_raw(lp.0 as *mut String) };
            app(|a| {
                let before = a.clean_before.take().unwrap_or(0);
                unsafe {
                    let _ = EnableWindow(a.c.clean, true);
                }
                a.mem = memory::read();
                a.invalidate_gauge();
                a.status(match wp.0 {
                    0 => format!(
                        "RAM libérée : {}",
                        fmt_gb(before.saturating_sub(a.mem.used))
                    ),
                    1 => "Opération annulée".into(),
                    _ => format!("Échec du vidage de la RAM ({err})"),
                });
            });
            return LRESULT(0);
        }
        WM_APP_RESTART => {
            app(|a| a.exit_code = RESTART);
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return LRESULT(0);
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return LRESULT(0);
        }
        WM_DESTROY => {
            let code = app(|a| {
                if a.shot.is_none() {
                    a.save_window();
                }
                a.exit_code
            });
            unsafe { PostQuitMessage(code.unwrap_or(0)) };
            return LRESULT(0);
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}
