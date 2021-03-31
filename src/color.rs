macro_rules! fg_color {
    ($r:expr, $g:expr, $b:expr) => {
        concat!("\x1b[38;2;", $r, ";", $g, ";", $b, "m").as_bytes()
    };
}

macro_rules! bg_color {
    ($r:expr, $g:expr, $b:expr) => {
        concat!("\x1b[48;2;", $r, ";", $g, ";", $b, "m").as_bytes()
    };
}

macro_rules! fg_color256 {
    ($number:expr) => {
        concat!("\x1b[38;5;", $number, "m").as_bytes()
    };
}

macro_rules! bg_color256 {
    ($number:expr) => {
        concat!("\x1b[48;5;", $number, "m").as_bytes()
    };
}

macro_rules! fg_color16 {
    (black) => {
        b"\x1b[30m"
    };
    (red) => {
        b"\x1b[31m"
    };
    (green) => {
        b"\x1b[32m"
    };
    (yellow) => {
        b"\x1b[33m"
    };
    (blue) => {
        b"\x1b[34m"
    };
    (magenta) => {
        b"\x1b[35m"
    };
    (cyan) => {
        b"\x1b[36m"
    };
    (white) => {
        b"\x1b[37m"
    };
    (bright_black) => {
        b"\x1b[90m"
    };
    (bright_red) => {
        b"\x1b[91m"
    };
    (bright_green) => {
        b"\x1b[92m"
    };
    (bright_yellow) => {
        b"\x1b[93m"
    };
    (bright_blue) => {
        b"\x1b[94m"
    };
    (bright_magenta) => {
        b"\x1b[95m"
    };
    (bright_cyan) => {
        b"\x1b[96m"
    };
    (bright_white) => {
        b"\x1b[97m"
    };
}

macro_rules! bg_color16 {
    (black) => {
        b"\x1b[40m"
    };
    (red) => {
        b"\x1b[41m"
    };
    (green) => {
        b"\x1b[42m"
    };
    (yellow) => {
        b"\x1b[43m"
    };
    (blue) => {
        b"\x1b[44m"
    };
    (magenta) => {
        b"\x1b[45m"
    };
    (cyan) => {
        b"\x1b[46m"
    };
    (white) => {
        b"\x1b[47m"
    };
    (bright_black) => {
        b"\x1b[100m"
    };
    (bright_red) => {
        b"\x1b[101m"
    };
    (bright_green) => {
        b"\x1b[102m"
    };
    (bright_yellow) => {
        b"\x1b[103m"
    };
    (bright_blue) => {
        b"\x1b[104m"
    };
    (bright_magenta) => {
        b"\x1b[105m"
    };
    (bright_cyan) => {
        b"\x1b[106m"
    };
    (bright_white) => {
        b"\x1b[107m"
    };
}
