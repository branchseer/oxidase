// https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#table-line-terminator-code-points
pub const LINE_TERMINATORS: &[&[u8]] = &[b"\n", b"\r", &[226, 128, 168], &[226, 128, 169]];

pub fn contains_line_terminators(buf: &[u8]) -> bool {
    for i in 0..buf.len() {
        for line_terminator in LINE_TERMINATORS {
            if buf[i..].starts_with(line_terminator) {
                return true;
            }
        }
    }
    false
}
