//! Every constant says whether it is solid (a cited rule or a measurement)
//! or assumed (authored). From TD/tests/consts_tagged.rs, changed to read
//! the first line of a doc block: cabin's comments run to several lines.

#[test]
fn every_constant_is_tagged() {
    let src = include_str!("../src/consts.rs");
    let lines: Vec<&str> = src.lines().collect();
    let mut untagged = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("pub const") {
            let mut top = k;
            while top > 0 && lines[top - 1].trim_start().starts_with("///") {
                top -= 1;
            }
            let first = if top < k { lines[top].trim_start() } else { "" };
            if !(first.starts_with("/// solid:") || first.starts_with("/// assumed:")) {
                untagged.push(l.trim());
            }
        }
    }
    assert!(untagged.is_empty(), "untagged constants: {untagged:#?}");
}
