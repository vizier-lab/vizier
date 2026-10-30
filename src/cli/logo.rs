const GREEN: &str = "\x1b[38;2;16;185;129m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn print_logo() {
    let big = format!("{GREEN}{BOLD}▄ {RESET}");
    let sm = format!("{GREEN}{DIM}. {RESET}");

    println!();
    println!("\t\t{big}{big}{sm}{sm}{sm}{sm}{big}{big}");
    println!("\t\t{big}{big}{sm}{sm}{sm}{sm}{big}{big}");
    println!("\t\t{big}{big}{big}{sm}{sm}{big}{big}{big}");
    println!("\t\t{sm}{big}{big}{sm}{sm}{big}{big}{sm}");
    println!("\t\t{sm}{big}{big}{sm}{sm}{big}{big}{sm}");
    println!("\t\t{sm}{sm}{big}{big}{big}{big}{sm}{sm}");
    println!("\t\t{sm}{sm}{big}{big}{big}{big}{sm}{sm}");
    println!("\t\t{sm}{sm}{sm}{big}{big}{sm}{sm}{sm}");
    println!();
    println!("\t    {GREEN}Vizier{RESET}  {DIM}AI Agent Framework{RESET}");
    println!();
}
