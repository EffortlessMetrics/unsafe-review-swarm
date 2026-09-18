use super::super::syntax::SyntaxNodeFact;
use crate::domain::OperationFamily;

/// Runtime disposition of one operation family for one syntax node.
///
/// A structural positive (`Detected`) or structural rejection (`CleanMiss`)
/// is authoritative: text fallback must not resurrect the same candidate and
/// family. Only `Unsupported` and `ParseFailed` regions may enter bounded
/// text fallback, and that entry is recorded, never silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FamilyDisposition {
    Detected,
    CleanMiss { reason: &'static str },
    Unsupported { reason: &'static str },
    ParseFailed { reason: &'static str },
}

/// The disposition of the `NonNullUnchecked` question for one call node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NonNullDecision {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) line: usize,
    pub(crate) end_line: usize,
    pub(crate) disposition: FamilyDisposition,
}

/// Why text fallback did or did not emit a card for one family on one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FallbackEntry {
    pub(crate) family: OperationFamily,
    pub(crate) line: usize,
    pub(crate) entered: bool,
    pub(crate) reason: &'static str,
}

pub(crate) struct NonNullDecisions {
    decisions: Vec<NonNullDecision>,
    pub(crate) parse_failed: bool,
}

/// Bundled inputs for text fallback so the syntax-first dispatch decision
/// travels with the syntax coverage it constrains.
pub(crate) struct FallbackDispatch<'a> {
    pub(crate) syntax_sites: &'a [super::DetectedSyntaxSite],
    pub(crate) syntax_index: &'a super::syntax_scan::SyntaxSiteIndex,
    pub(crate) nonnull: &'a NonNullDecisions,
    pub(crate) entries: Vec<FallbackEntry>,
}

impl NonNullDecisions {
    pub(crate) fn disposition_for_node(
        &self,
        start: usize,
        end: usize,
    ) -> Option<FamilyDisposition> {
        self.decisions
            .iter()
            .find(|decision| decision.start == start && decision.end == end)
            .map(|decision| decision.disposition)
    }

    /// Whether a `NonNullUnchecked` text hit on this line redetects without
    /// the `NonNull` rule instead of emitting. True when a clean miss covers
    /// the line, except when a covering clean miss merely overlaps a
    /// detection without being nested inside it (a wrapping call such as
    /// `Some(unsafe { NonNull::new_unchecked(p) })`): there the fallback hit
    /// is the genuine outer-span card. A clean miss strictly nested inside a
    /// detection (a homonym argument) redetects, so the rejected hit is
    /// never re-added; disjoint same-line siblings redetect as before.
    pub(crate) fn clean_miss_redetects_line(&self, line: usize) -> bool {
        let detected: Vec<(usize, usize)> = self
            .decisions
            .iter()
            .filter(|decision| matches!(decision.disposition, FamilyDisposition::Detected))
            .map(|decision| (decision.start, decision.end))
            .collect();
        let mut any_covering = false;
        for decision in &self.decisions {
            if !matches!(decision.disposition, FamilyDisposition::CleanMiss { .. }) {
                continue;
            }
            if !(decision.line <= line && line <= decision.end_line) {
                continue;
            }
            any_covering = true;
            let overlaps_detection = detected
                .iter()
                .any(|(start, end)| decision.start < *end && *start < decision.end);
            let nested_in_detection = detected
                .iter()
                .any(|(start, end)| *start < decision.start && decision.end < *end);
            if overlaps_detection && !nested_in_detection {
                return false;
            }
        }
        any_covering
    }

    pub(crate) fn unsupported_covers_line(&self, line: usize) -> bool {
        self.decision_covers_line(line, |disposition| {
            matches!(disposition, FamilyDisposition::Unsupported { .. })
        })
    }

    pub(crate) fn parse_failed_covers_line(&self, line: usize) -> bool {
        self.decision_covers_line(line, |disposition| {
            matches!(disposition, FamilyDisposition::ParseFailed { .. })
        })
    }

    fn decision_covers_line(
        &self,
        line: usize,
        matches_kind: impl Fn(FamilyDisposition) -> bool,
    ) -> bool {
        self.decisions.iter().any(|decision| {
            matches_kind(decision.disposition) && decision.line <= line && line <= decision.end_line
        })
    }
}

/// Decide the `NonNullUnchecked` question structurally for every call node.
///
/// The callee path is read from the parsed call node (last two `::`
/// segments must be exactly `NonNull` + `new_unchecked`), not from a line
/// substring, so a same-named path on another type (`my_NonNull`,
/// `Pin`, `Foo`) is a structural clean miss rather than a detection.
/// Unexpanded macro content is unsupported: it is never claimed as analyzed.
pub(crate) fn nonnull_call_decisions(
    nodes: &[SyntaxNodeFact],
    unsafe_block_ranges: &[(usize, usize)],
    unsafe_fn_ranges: &[(usize, usize)],
    parse_failed: bool,
) -> NonNullDecisions {
    let mut decisions = Vec::new();
    for fact in nodes {
        if !snippet_mentions_new_unchecked(&fact.snippet) {
            continue;
        }
        let end_line = fact.line + fact.snippet.lines().count().saturating_sub(1);
        if fact.kind == "ERROR" {
            decisions.push(NonNullDecision {
                start: fact.start,
                end: fact.end,
                line: fact.line,
                end_line,
                disposition: FamilyDisposition::ParseFailed {
                    reason: "unparsed syntax region",
                },
            });
            continue;
        }
        // Only `CALL_EXPR` can be the associated-function call itself: a
        // `METHOD_CALL_EXPR` whose snippet mentions `new_unchecked` is a
        // method chained on the constructed value (e.g.
        // `NonNull::new_unchecked(p).as_ptr()`), and classifying it as the
        // `NonNull` call records a duplicate, wrong decision for the outer
        // call. The inner `CALL_EXPR` fact already carries the detection.
        let is_call = fact.kind == "CALL_EXPR";
        let is_macro = fact.kind == "MACRO_EXPR";
        if !is_call && !is_macro {
            continue;
        }
        if is_macro {
            decisions.push(NonNullDecision {
                start: fact.start,
                end: fact.end,
                line: fact.line,
                end_line,
                disposition: FamilyDisposition::Unsupported {
                    reason: "unexpanded macro content",
                },
            });
            continue;
        }
        let in_scope =
            is_inside_range(fact, unsafe_block_ranges) || is_inside_range(fact, unsafe_fn_ranges);
        if !in_scope {
            decisions.push(NonNullDecision {
                start: fact.start,
                end: fact.end,
                line: fact.line,
                end_line,
                disposition: FamilyDisposition::CleanMiss {
                    reason: "call is outside unsafe scope",
                },
            });
            continue;
        }
        if callee_is_nonnull_new_unchecked(&fact.snippet) {
            decisions.push(NonNullDecision {
                start: fact.start,
                end: fact.end,
                line: fact.line,
                end_line,
                disposition: FamilyDisposition::Detected,
            });
        } else {
            decisions.push(NonNullDecision {
                start: fact.start,
                end: fact.end,
                line: fact.line,
                end_line,
                disposition: FamilyDisposition::CleanMiss {
                    reason: "callee path is not NonNull::new_unchecked",
                },
            });
        }
    }
    NonNullDecisions {
        decisions,
        parse_failed,
    }
}

fn snippet_mentions_new_unchecked(snippet: &str) -> bool {
    snippet.contains("new_unchecked")
}

fn is_inside_range(fact: &SyntaxNodeFact, ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|(start, end)| fact.start >= *start && fact.end <= *end)
}

/// Read the callee path structurally from a call-node snippet: the text
/// before the first top-level `(`, with any `::<...>` turbofish removed.
/// Only an exact trailing `NonNull::new_unchecked` path segment pair counts;
/// `my_NonNull::new_unchecked`, `Pin::new_unchecked`, and method calls on
/// other receivers do not.
fn callee_is_nonnull_new_unchecked(snippet: &str) -> bool {
    let Some(callee) = callee_before_call_paren(snippet) else {
        return false;
    };
    let callee = callee.trim();
    let path = callee
        .split_once("::<")
        .map_or(callee, |(path, _generics)| path)
        .trim();
    let mut segments: Vec<&str> = path.split("::").collect();
    if let Some(last) = segments.last() {
        let receiver = last.split('.').next_back().unwrap_or(last);
        if receiver != "new_unchecked" {
            return false;
        }
        if last.contains('.') {
            segments.pop();
            segments.push(receiver);
        }
    }
    let [.., parent, name] = segments.as_slice() else {
        return false;
    };
    *name == "new_unchecked" && *parent == "NonNull"
}

/// Text before the first top-level `(` of a call-node snippet.
fn callee_before_call_paren(snippet: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (idx, ch) in snippet.char_indices() {
        match ch {
            '(' | '[' => {
                if ch == '(' && depth == 0 {
                    return Some(&snippet[..idx]);
                }
                depth += 1;
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::callee_is_nonnull_new_unchecked;

    #[test]
    fn callee_check_accepts_plain_and_turbofish_paths() {
        assert!(callee_is_nonnull_new_unchecked("NonNull::new_unchecked(p)"));
        assert!(callee_is_nonnull_new_unchecked(
            "NonNull::new_unchecked::<u8>(p)"
        ));
        assert!(callee_is_nonnull_new_unchecked(
            "core::ptr::NonNull::new_unchecked(p)"
        ));
    }

    #[test]
    fn chained_method_call_gets_no_nonnull_decision() {
        use crate::analysis::syntax::SyntaxNodeFact;

        fn fact(kind: &str, start: usize, end: usize, snippet: &str) -> SyntaxNodeFact {
            SyntaxNodeFact {
                kind: kind.to_string(),
                start,
                end,
                line: 4,
                column: 1,
                end_line: 4,
                end_column: 1,
                snippet: snippet.to_string(),
            }
        }
        let nodes = vec![
            fact(
                "METHOD_CALL_EXPR",
                78,
                112,
                "NonNull::new_unchecked(p).as_ptr()",
            ),
            fact("CALL_EXPR", 78, 103, "NonNull::new_unchecked(p)"),
        ];
        let decisions = super::nonnull_call_decisions(&nodes, &[(0, usize::MAX)], &[], false);
        assert!(decisions.disposition_for_node(78, 112).is_none());
        assert!(decisions.disposition_for_node(78, 103).is_some());
    }

    #[test]
    fn callee_check_rejects_homonym_and_other_receivers() {
        assert!(!callee_is_nonnull_new_unchecked(
            "my_NonNull::new_unchecked(p)"
        ));
        assert!(!callee_is_nonnull_new_unchecked("Pin::new_unchecked(v)"));
        assert!(!callee_is_nonnull_new_unchecked("Foo::new_unchecked(x)"));
        assert!(!callee_is_nonnull_new_unchecked("p.new_unchecked()"));
        assert!(!callee_is_nonnull_new_unchecked("NonNull::dangling()"));
    }
}
