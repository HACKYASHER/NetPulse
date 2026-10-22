//! Read the level field, never severity-looking substrings in paths or messages.
pub fn complete_tail(text: &str, limit: usize) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    let mut lines: Vec<&str> = clean.split_inclusive('\n').collect();
    if !clean.ends_with('\n') {
        lines.pop();
    }
    let mut bytes = 0;
    let mut kept = Vec::new();
    for line in lines.into_iter().rev().take(64) {
        if bytes + line.len() > limit {
            break;
        }
        bytes += line.len();
        kept.push(line);
    }
    kept.into_iter().rev().collect()
}
pub fn level(line: &str) -> &'static str {
    for token in line.split_whitespace().take(5) {
        match token.trim_matches(['[', ']']).to_ascii_uppercase().as_str() {
            "ERROR" | "FATAL" | "PANIC" => return "ERROR",
            "WARN" | "WARNING" => return "WARNING",
            "INFO" => return "INFO",
            "DEBUG" => return "DEBUG",
            "TRACE" => return "TRACE",
            _ => {}
        }
    }
    "INFO"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_tail_is_utf8_bounded_and_never_replays_partial_lines() {
        assert_eq!(
            complete_tail("old error\nWARNING second\npartial", 18),
            "WARNING second\n"
        );
        assert_eq!(complete_tail("первая\nвторая\n", 13), "вторая\n");
        assert_eq!(complete_tail(&"x".repeat(7000), 6000), "");
        assert_eq!(complete_tail(&("x".repeat(7000) + "\n"), 6000), "");
    }
    #[test]
    fn real_core_formats_do_not_confuse_module_names_with_levels() {
        assert_eq!(
            level(
                "2026/10/21 19:49:54.007554 [Warning] common/errors: The feature gRPC transport is deprecated"
            ),
            "WARNING"
        );
        assert_eq!(
            level("2026/10/21 19:49:54.009555 [Warning] core: Xray started"),
            "WARNING"
        );
        assert_eq!(
            level(
                "+1000 2026-10-21 22:00:13 ERROR [3103931491 0ms] connection: unreachable network"
            ),
            "ERROR"
        );
        assert_eq!(
            level("2026/10/21 19:49:54.007055 [Info] common/errors: no failed calls"),
            "INFO"
        );
        assert_eq!(
            level("from tcp:127.0.0.1:59408 accepted tcp:error.example:443"),
            "INFO"
        );
        assert_eq!(level("Xray: Xray 26.10.17"), "INFO");
    }
}
