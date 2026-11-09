//! Output conventions shared by every command (docs/aodm-cli.md §3): results on stdout
//! as text or one `--json` document, progress on stderr, and the exit codes.

use serde_json::{Value, json};

/// JSON document format; changes only on incompatible changes (noted in CHANGELOG).
pub const SCHEMA: u32 = 1;

pub const REFUSED: i32 = 3;
pub const PARTIAL: i32 = 4;
pub const TIMEOUT: i32 = 5;

pub struct Out {
    pub json: bool,
    results: Vec<Value>,
}

/// Some targets failed, the rest succeeded (exit 4).
#[derive(Debug)]
pub struct Partial(pub String);

impl std::fmt::Display for Partial {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Partial {}

/// Every target failed the same way (all refused: 3, all timed out: 5).
#[derive(Debug)]
pub struct Coded(pub i32, pub String);

impl std::fmt::Display for Coded {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for Coded {}

/// Exit code of one core error.
pub fn core_code(e: &aod_core::Error) -> i32 {
    match e {
        aod_core::Error::Refused(_) => REFUSED,
        aod_core::Error::Timeout { .. } => TIMEOUT,
        _ => 1,
    }
}

/// Usage error found after parsing (exit 2), e.g. `--dry-run` on a command without it.
#[derive(Debug)]
pub struct Usage(pub String);

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Usage {}

impl Out {
    pub fn new(json: bool) -> Out {
        Out {
            json,
            results: Vec::new(),
        }
    }

    /// One result: `text` goes to stdout (unless --json), `v` into the JSON results.
    pub fn line(&mut self, text: impl AsRef<str>, v: Value) {
        if !self.json {
            emit(&format!("{}\n", text.as_ref()));
        }
        self.results.push(v);
    }

    /// A rendered table (or block) and the rows behind it.
    pub fn table(&mut self, text: &str, rows: impl IntoIterator<Item = Value>) {
        if !self.json {
            emit(text);
        }
        self.results.extend(rows);
    }

    /// A JSON result with no stdout text (failures go to stderr instead).
    pub fn push(&mut self, v: Value) {
        self.results.push(v);
    }

    /// Text only (headers, notes on stdout that have no JSON counterpart).
    pub fn text(&self, text: impl AsRef<str>) {
        if !self.json {
            emit(&format!("{}\n", text.as_ref()));
        }
    }

    /// Prints the JSON document (when --json) for `command` and the final error.
    pub fn finish(self, command: &str, err: Option<&anyhow::Error>) {
        if self.json {
            let errors: Vec<Value> = err
                .map(|e| json!({"message": format!("{e:#}"), "code": code(e)}))
                .into_iter()
                .collect();
            let doc = json!({
                "schema": SCHEMA,
                "command": command,
                "ok": err.is_none(),
                "results": self.results,
                "errors": errors,
            });
            emit(&format!("{doc:#}\n"));
        }
    }
}

/// Writes to stdout. A closed pipe (`aodm … | head`) is not an error: the command
/// still finishes its work, it just stops printing.
pub fn emit(s: &str) {
    use std::io::Write;
    let _ = std::io::stdout().lock().write_all(s.as_bytes());
}

/// Exit code for an error: refused 3, partial 4, timeout 5, usage 2, else 1.
pub fn code(e: &anyhow::Error) -> i32 {
    for cause in e.chain() {
        if cause.is::<Partial>() {
            return PARTIAL;
        }
        if cause.is::<Usage>() {
            return 2;
        }
        if let Some(c) = cause.downcast_ref::<Coded>() {
            return c.0;
        }
        if let Some(e) = cause.downcast_ref::<aod_core::Error>()
            && core_code(e) != 1
        {
            return core_code(e);
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        let refused: anyhow::Error = aod_core::Error::Refused("x".into()).into();
        assert_eq!(code(&refused), REFUSED);
        assert_eq!(code(&refused.context("while saving")), REFUSED);
        let t: anyhow::Error = aod_core::Error::Timeout {
            what: "boot".into(),
            secs: 1,
        }
        .into();
        assert_eq!(code(&t), TIMEOUT);
        assert_eq!(code(&Partial("1 of 2".into()).into()), PARTIAL);
        assert_eq!(code(&Usage("x".into()).into()), 2);
        assert_eq!(code(&Coded(5, "x".into()).into()), TIMEOUT);
        assert_eq!(code(&anyhow::anyhow!("boom")), 1);
    }
}
