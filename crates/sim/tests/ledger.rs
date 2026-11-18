//! Every rule the sim cites must still say what the sim says it does.
//!
//! A citation is written `C3-143 "phrase"` anywhere in `src/`. This test
//! reads `rails/research.md`, the ledger the rules come from, and fails if a
//! cited row is missing or no longer holds its phrase. It never skips: a
//! missing ledger fails it (plan, edges table), because a simulator checked
//! only against its own code proves nothing.

use std::path::Path;

#[test]
fn every_cited_ledger_row_holds_its_phrase() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // CABIN_LEDGER points at an edited copy for the failure drill in
    // tools/verify_b8.py; `rails/` itself is never written (plan, out of scope).
    let path = std::env::var_os("CABIN_LEDGER").map_or_else(|| root.join("../../../../rails/research.md"), Into::into);
    let ledger = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}. rails/research.md must sit beside games/: the rules are checked against it", path.display()));
    let mut cited = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(root.join("src")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        // Comments wrap; join each file into one line so a citation split
        // across two comment lines still reads as one.
        let flat: String = text.lines().map(|l| l.trim_start().trim_start_matches("//!").trim_start_matches("///").trim_start_matches("//").trim()).collect::<Vec<_>>().join(" ");
        let mut rest = flat.as_str();
        while let Some(i) = rest.find(" \"") {
            let (before, after) = rest.split_at(i);
            let id = before.rsplit(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            let after = &after[2..];
            let Some(j) = after.find('"') else { break };
            let ok_id = id.len() >= 4 && id.as_bytes()[0] == b'C' && id[1..].split_once('-').is_some_and(|(a, b)| {
                !a.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit())
            });
            if ok_id {
                cited.push((f.file_name().unwrap().to_string_lossy().to_string(), id.to_string(), after[..j].to_string()));
            }
            rest = &after[j + 1..];
        }
    }
    assert!(cited.len() >= 5, "found only {} citations: {cited:?}", cited.len());
    for (file, id, phrase) in &cited {
        let row = ledger
            .lines()
            .find(|l| l.starts_with(&format!("| {id} |")))
            .unwrap_or_else(|| panic!("{file} cites {id}, which rails/research.md does not have"));
        assert!(row.contains(phrase.as_str()), "{file} cites {id} \"{phrase}\", but the row now reads: {row}");
    }
}
