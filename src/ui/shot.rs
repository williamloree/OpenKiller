//! `--screenshot`: renders the main window (and an open dialog on top) into a PNG via PrintWindow + WIC.

use std::path::Path;
use windows::Win32::Foundation::{GENERIC_WRITE, HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows::core::{HSTRING, Result};

/// Visible bounds, without the invisible resize borders.
fn frame(h: HWND) -> RECT {
    let mut r = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as _,
            size_of::<RECT>() as u32,
        )
        .is_err()
        {
            let _ = GetWindowRect(h, &mut r);
        }
    }
    r
}

pub fn capture(main: HWND, dialog: Option<HWND>, path: &Path) -> Result<()> {
    let out = frame(main);
    let (w, h) = (out.right - out.left, out.bottom - out.top);
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp.into());
        for win in std::iter::once(main).chain(dialog) {
            let mut wr = RECT::default();
            let _ = GetWindowRect(win, &mut wr);
            let (ww, wh) = (wr.right - wr.left, wr.bottom - wr.top);
            let tdc = CreateCompatibleDC(Some(screen));
            let tbmp = CreateCompatibleBitmap(screen, ww, wh);
            let told = SelectObject(tdc, tbmp.into());
            // PW_RENDERFULLCONTENT (2): DWM content, dark title bar included.
            let _ = PrintWindow(win, tdc, PRINT_WINDOW_FLAGS(2));
            let fr = frame(win);
            let _ = BitBlt(
                dc,
                fr.left - out.left,
                fr.top - out.top,
                fr.right - fr.left,
                fr.bottom - fr.top,
                Some(tdc),
                fr.left - wr.left,
                fr.top - wr.top,
                SRCCOPY,
            );
            SelectObject(tdc, told);
            let _ = DeleteObject(tbmp.into());
            let _ = DeleteDC(tdc);
        }
        SelectObject(dc, old);
        let r = save_png(bmp, w as u32, h as u32, path);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        r
    }
}

fn save_png(bmp: HBITMAP, w: u32, h: u32, path: &Path) -> Result<()> {
    unsafe {
        let f: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bitmap = f.CreateBitmapFromHBITMAP(bmp, HPALETTE::default(), WICBitmapIgnoreAlpha)?;
        let stream = f.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
        let enc = f.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let (mut frame, mut props) = (None, None);
        enc.CreateNewFrame(&mut frame, &mut props)?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(props.as_ref())?;
        frame.SetSize(w, h)?;
        let mut fmt = GUID_WICPixelFormat24bppBGR;
        frame.SetPixelFormat(&mut fmt)?;
        frame.WriteSource(&bitmap, std::ptr::null())?;
        frame.Commit()?;
        enc.Commit()
    }
}
