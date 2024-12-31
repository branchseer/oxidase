// https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#table-line-terminator-code-points
pub const LINE_TERMINATORS: &[&[u8]] = &[b"\n", b"\r", &[226, 128, 168], &[226, 128, 169]];

pub fn line_terminator_start_iter(buf: &[u8]) -> impl Iterator<Item = usize> + use<'_> {
    (0..buf.len()).filter(|start| {
        for line_terminator in LINE_TERMINATORS {
            if buf[*start..].starts_with(line_terminator) {
                return true;
            }
        }
        false
    })
}

pub fn contains_line_terminators(buf: &[u8]) -> bool {
    line_terminator_start_iter(buf).next().is_some()
}
