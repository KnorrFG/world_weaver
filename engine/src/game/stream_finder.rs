pub struct StreamFinder {
    target: Vec<char>,
    current_pos: usize,
    stored_output: Vec<char>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchResult {
    Blocked,
    StopTokenMatched {
        pre_token_text: String,
        post_token_text: String,
    },
    CheckedOutput(String),
}

#[derive(Debug, PartialEq, Eq)]
enum ProcessCharOutcome {
    PrefixMatch,
    PrefixReMatch,
    Mismatch,
    FullMatch,
}

impl StreamFinder {
    pub fn new(target: &'static str) -> Self {
        Self {
            target: target.chars().collect(),
            current_pos: 0,
            stored_output: vec![],
        }
    }

    fn process_char(&mut self, ch: char) -> ProcessCharOutcome {
        if ch == self.target[self.current_pos] {
            self.current_pos += 1;
            if self.current_pos == self.target.len() {
                ProcessCharOutcome::FullMatch
            } else {
                ProcessCharOutcome::PrefixMatch
            }
        } else if ch == self.target[0] {
            self.current_pos = 1;
            ProcessCharOutcome::PrefixReMatch
        } else {
            self.current_pos = 0;
            ProcessCharOutcome::Mismatch
        }
    }

    fn reset(&mut self) {
        self.current_pos = 0;
        self.stored_output.clear();
    }

    pub fn process(&mut self, input: &str) -> MatchResult {
        let mut output_chars = vec![];
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            match self.process_char(ch) {
                ProcessCharOutcome::Mismatch => {
                    if self.stored_output.is_empty() {
                        output_chars.push(ch);
                    } else {
                        output_chars.extend_from_slice(&self.stored_output);
                        output_chars.push(ch);
                        self.stored_output.clear();
                    }
                }
                ProcessCharOutcome::PrefixMatch => {
                    self.stored_output.push(ch);
                }
                ProcessCharOutcome::PrefixReMatch => {
                    output_chars.extend_from_slice(&self.stored_output);
                    self.stored_output.clear();
                    self.stored_output.push(ch);
                }
                ProcessCharOutcome::FullMatch => {
                    self.reset();
                    return MatchResult::StopTokenMatched {
                        pre_token_text: output_chars.into_iter().collect(),
                        post_token_text: chars.collect(),
                    };
                }
            }
        }

        if output_chars.is_empty() {
            MatchResult::Blocked
        } else {
            MatchResult::CheckedOutput(output_chars.into_iter().collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MatchResult, StreamFinder};

    #[test]
    fn test_stop_token_detection() {
        let mut matcher = StreamFinder::new("User:");
        assert_eq!(
            matcher.process("A User is an idiot"),
            MatchResult::CheckedOutput("A User is an idiot".into()),
        );
        assert_eq!(
            matcher.process("A User"),
            MatchResult::CheckedOutput("A ".into()),
        );
        assert_eq!(
            matcher.process(" is"),
            MatchResult::CheckedOutput("User is".into()),
        );

        assert_eq!(
            matcher.process("A User: is an"),
            MatchResult::StopTokenMatched {
                pre_token_text: "A ".into(),
                post_token_text: " is an".into()
            },
        );

        assert_eq!(
            matcher.process("User:"),
            MatchResult::StopTokenMatched {
                pre_token_text: "".into(),
                post_token_text: "".into()
            },
        );

        assert_eq!(
            matcher.process("UsUser:"),
            MatchResult::StopTokenMatched {
                pre_token_text: "Us".into(),
                post_token_text: "".into(),
            },
        );
    }
}
