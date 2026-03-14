use ratatui::style::Color;

pub const ART: &str = r#"
⠀⠀⣠⠤⠖⠒⠦⢤⡀⠀⠀⠀⠀⠀⠀⢀⠤⠴⠒⠢⠤⣀⠀⠀
⠀⣼⠁⠀⠀⡠⢖⡉⠁⠀⠀⠀⠀⠀⠀⠈⢙⡲⣄⠀⠀⠈⣇⠀
⠀⣟⣄⠀⠐⠓⢋⡇⠀⠀⠀⠀⠀⠀⠀⠀⢹⡙⠚⠀⠀⡠⣻⠀
⠀⠈⡶⢭⣒⡺⠟⣀⣰⣿⠦⠤⠤⢼⣿⣆⡈⠻⢖⣒⡭⡾⠁⠀
⠀⠀⠱⡘⢄⡰⠊⠁⠀⠀⠀⠀⠀⠀⠀⠀⠈⠑⣄⡰⣃⠇⠀⠀
⠀⣀⠤⠬⢽⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸⡯⠥⠤⡀⠀
⠰⠕⢋⡭⠿⡟⢄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⣻⠯⢭⡙⠺⠆
⠀⢰⡳⠊⡩⠛⣦⡉⠒⠤⠤⠄⠤⠤⠤⠒⢉⣔⠛⢍⠓⣝⡄⠀
⠀⠈⠁⡼⡴⠉⠀⠈⠓⠲⠤⠤⠤⠤⠖⠚⠁⠈⠉⣎⣧⠈⠁⠀
⠀⠀⠀⠉⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠁⠀⠀⠀
"#;

const WAVE_PALETTE: [Color; 6] = [
    Color::Rgb(6, 182, 212),
    Color::Rgb(22, 198, 228),
    Color::Rgb(46, 212, 238),
    Color::Rgb(22, 198, 228),
    Color::Rgb(6, 182, 212),
    Color::Rgb(8, 145, 178),
];

const CELEBRATE_PALETTE: [Color; 6] = [
    Color::Rgb(34, 197, 94),
    Color::Rgb(6, 182, 212),
    Color::Rgb(34, 197, 94),
    Color::Rgb(6, 182, 212),
    Color::Rgb(34, 197, 94),
    Color::Rgb(6, 182, 212),
];

pub fn line_color(line_idx: usize, tick: usize, celebrating: bool) -> Color {
    let palette = if celebrating { &CELEBRATE_PALETTE } else { &WAVE_PALETTE };
    let offset = tick / 2;
    palette[(line_idx + offset) % palette.len()]
}

pub fn bob_offset(tick: usize, celebrating: bool) -> u16 {
    if celebrating {
        return 0;
    }
    match (tick / 4) % 4 {
        0 | 2 => 0,
        1 => 1,
        _ => 0,
    }
}
