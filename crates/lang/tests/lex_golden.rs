//! Golden tests for the lexer: every `golden/lex/NAME.spell` is tokenized and
//! compared with `golden/lex/NAME.tokens`. Run with `BLESS=1` to rewrite them.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use lang::lexer::tokenize;

fn render(src: &str) -> String {
    match tokenize(src) {
        Ok(tokens) => tokens.iter().fold(String::new(), |mut out, t| {
            writeln!(out, "{}:{} {:?}", t.pos.line, t.pos.col, t.kind).unwrap();
            out
        }),
        Err(e) => format!(
            "{}:{} error {} {:?}\n",
            e.pos.line,
            e.pos.col,
            e.code(),
            e.kind
        ),
    }
}

#[test]
fn lex_golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/lex");
    let bless = std::env::var_os("BLESS").is_some();
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "spell") {
            continue;
        }
        count += 1;
        let actual = render(&fs::read_to_string(&path).unwrap());
        let expected_path = path.with_extension("tokens");
        if bless {
            fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                path.display()
            ));
        }
    }
    assert!(count > 0, "no golden files in {}", dir.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
