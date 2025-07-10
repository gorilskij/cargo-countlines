pub fn format_number(num: usize) -> String {
    let s = num.to_string();
    let mut x = s.len() % 3 + 3; // the `+ 3` avoids a leading comma
    let mut out = String::new();
    for c in s.chars() {
        if x == 0 {
            out.push(',');
        }
        x = (x + 2) % 3; // x - 1 (mod 3)
        out.push(c);
    }
    out
}
