// Release derlemesinde konsol penceresi açılmaz.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dim_desktop_lib::run();
}
