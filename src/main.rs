#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod demo;
mod export;
mod memory;
mod model;
mod ports;
mod process;
mod ui;
mod updater;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

const USAGE: &str = "Usage : openkiller [--demo | --screenshot <fichier.png> [--view ports|favorites|kill|presets] | update]";

/// Release builds have no console: borrow the parent terminal for CLI output.
fn console() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Elevated helper: no other argument accepted, nothing read.
        Some("clean-ram") => std::process::exit(if args.len() == 1 {
            memory::clean_ram();
            0
        } else {
            2
        }),
        Some("update") => {
            console();
            updater::cleanup();
            match updater::check_and_install(true) {
                Ok(updater::Outcome::UpToDate) => {
                    println!("OpenKiller est à jour (v{}).", updater::VERSION)
                }
                Ok(updater::Outcome::Installed(tag)) => {
                    println!("OpenKiller mis à jour : {tag}. Relance l'application.")
                }
                Err(e) => {
                    eprintln!("Mise à jour impossible : {e}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--screenshot") => {
            let view = args.get(2..).and_then(ui::View::parse);
            let (Some(png), Some(view)) = (args.get(1), view) else {
                console();
                eprintln!("{USAGE}");
                std::process::exit(2);
            };
            std::process::exit(ui::run(ui::Mode::Shot(png.into(), view)));
        }
        Some("--demo") if args.len() == 1 => std::process::exit(ui::run(ui::Mode::Demo)),
        None => {}
        _ => {
            console();
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }

    updater::cleanup();
    let mutex = unsafe { CreateMutexW(None, true, w!("Local\\openkiller-single-instance")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        ui::focus_existing();
        return;
    }
    let restart = ui::run(ui::Mode::Real) == ui::RESTART;
    // Release the mutex before relaunching, or the new instance would just focus this one.
    if let Ok(m) = mutex {
        unsafe {
            let _ = CloseHandle(m);
        }
    }
    if restart && let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).args(&args).spawn();
    }
}
