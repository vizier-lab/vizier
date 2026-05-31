const GREEN: &str = "\x1b[38;2;16;185;129m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn print_logo() {
    let big = format!("{GREEN}{BOLD}▄ {RESET}");
    let sm = format!("{GREEN}{DIM}. {RESET}");

    println!();
    println!("  {big}{big}{sm}{sm}{sm}{sm}{big}{big}");
    println!("  {big}{big}{sm}{sm}{sm}{sm}{big}{big}");
    println!("  {big}{big}{big}{sm}{sm}{big}{big}{big}");
    println!("  {sm}{big}{big}{sm}{sm}{big}{big}{sm}");
    println!("  {sm}{big}{big}{sm}{sm}{big}{big}{sm}");
    println!("  {sm}{sm}{big}{big}{big}{big}{sm}{sm}");
    println!("  {sm}{sm}{big}{big}{big}{big}{sm}{sm}");
    println!("  {sm}{sm}{sm}{big}{big}{sm}{sm}{sm}");
    println!();
    println!("  {GREEN}Vizier{RESET}  {DIM}AI Agent Framework{RESET}");
    println!();
}
