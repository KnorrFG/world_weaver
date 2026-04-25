//! Markdown import/export for world descriptions.

use std::{collections::BTreeMap, fmt::Write};

use color_eyre::Result;
use log::warn;

use crate::game::{PcDescription, WorldDescription};

const WORLD_MARKDOWN_FORMAT_VERSION: u32 = 1;

pub fn world_to_markdown(world: &WorldDescription) -> String {
    let mut out = String::new();
    writeln!(out, "<!-- WW:FORMAT {WORLD_MARKDOWN_FORMAT_VERSION} -->").unwrap();
    writeln!(out, "\n# {}\n", world.name).unwrap();
    write_heading_field(&mut out, "world.name");
    writeln!(out, "\n# Description\n").unwrap();
    write_block_field(&mut out, "world.description", &world.main_description);
    writeln!(out, "\n# Initial Action\n").unwrap();
    write_block_field(&mut out, "world.initial_action", &world.init_action);

    if !world.pc_descriptions.is_empty() {
        writeln!(out, "\n# Characters").unwrap();

        for (name, character) in &world.pc_descriptions {
            writeln!(out, "\n## {name}").unwrap();
            write_heading_field(&mut out, "character.name");
            write_character_start(&mut out);
            writeln!(out, "\n### Description\n").unwrap();
            write_block_field(&mut out, "character.description", &character.description);
            writeln!(out, "\n### Initial Action\n").unwrap();
            write_block_field(&mut out, "character.initial_action", &character.initial_action);
            write_character_end(&mut out);
        }
    }

    out
}

pub fn world_from_markdown(src: &str) -> Result<WorldDescription> {
    let name = first_heading_field(src, "world.name", 1);
    let main_description = first_field(src, "world.description");
    let init_action = first_field(src, "world.initial_action");

    let mut pc_descriptions = BTreeMap::new();

    for section in collect_character_blocks(src) {
        let character_name = first_heading_field(section, "character.name", 2);
        if !character_name.is_empty() {
            let description = first_field(section, "character.description");
            let initial_action = first_field(section, "character.initial_action");
            pc_descriptions.insert(
                character_name,
                PcDescription {
                    description,
                    initial_action,
                },
            );
        }
    }

    Ok(WorldDescription {
        name,
        main_description,
        pc_descriptions,
        init_action,
    })
}

fn write_heading_field(out: &mut String, key: &str) {
    writeln!(out, "<!-- WW:HEADING {key} -->").unwrap();
}

fn write_block_field(out: &mut String, key: &str, value: &str) {
    writeln!(out, "<!-- WW:FIELD {key} -->").unwrap();
    writeln!(out, "{value}").unwrap();
    writeln!(out, "<!-- /WW:FIELD {key} -->").unwrap();
}

fn write_character_start(out: &mut String) {
    writeln!(out, "<!-- WW:CHARACTER -->").unwrap();
}

fn write_character_end(out: &mut String) {
    writeln!(out, "<!-- /WW:CHARACTER -->").unwrap();
}

fn collect_fields(src: &str, key: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cursor = src;

    loop {
        let Some(start_idx) = cursor.find(&field_start_prefix(key)) else {
            return fields;
        };
        let after_start = &cursor[start_idx..];

        if let Some(header_end) = after_start.find("-->") {
            let header = &after_start[..header_end + 3];
            let after_header = &after_start[header_end + 3..];

            if let Some((value, rest)) = parse_field_value(header, after_header, key) {
                if let Some(value) = value {
                    fields.push(value);
                }
                cursor = rest;
            } else {
                return fields;
            }
        } else {
            cursor = after_start;
        }
    }
}

fn parse_field_value<'a>(
    header: &str,
    after_header: &'a str,
    key: &str,
) -> Option<(Option<String>, &'a str)> {
    if header == block_field_start_marker(key) {
        parse_block_field(after_header, key)
    } else {
        Some((parse_inline_field_header(header, key), after_header))
    }
}

fn parse_block_field<'a>(after_header: &'a str, key: &str) -> Option<(Option<String>, &'a str)> {
    let content = after_header.strip_prefix('\n')?;
    let end_marker = field_end_marker(key);
    let end_idx = content.find(&end_marker)?;
    let mut value = content[..end_idx].to_string();
    if value.ends_with('\n') {
        value.pop();
    }
    let rest = &content[end_idx + end_marker.len()..];
    Some((Some(value), rest))
}

fn first_field(src: &str, key: &str) -> String {
    let fields = collect_fields(src, key);
    if fields.len() > 1 {
        warn!("Found multiple markdown fields for {key}, using the first one");
    }
    fields.into_iter().next().unwrap_or_default()
}

fn first_heading_field(src: &str, key: &str, level: usize) -> String {
    let marker = heading_field_marker(key);
    let heading_prefix = "#".repeat(level) + " ";
    let mut headings = Vec::new();
    let mut cursor = src;

    loop {
        let Some(marker_idx) = cursor.find(&marker) else {
            if headings.len() > 1 {
                warn!("Found multiple heading fields for {key}, using the first one");
            }
            return headings.into_iter().next().unwrap_or_default();
        };

        let before_marker = &cursor[..marker_idx];
        if let Some(value) = last_heading(before_marker, &heading_prefix) {
            headings.push(value);
        }
        cursor = &cursor[marker_idx + marker.len()..];
    }
}

fn collect_character_blocks(src: &str) -> Vec<&str> {
    let start_marker = "<!-- WW:CHARACTER -->";
    let end_marker = "<!-- /WW:CHARACTER -->";

    let mut blocks = Vec::new();
    let mut cursor = src;

    loop {
        let Some(start_idx) = cursor.find(start_marker) else {
            return blocks;
        };
        let after_start = &cursor[start_idx + start_marker.len()..];

        if let Some(content) = after_start.strip_prefix('\n') {
            if let Some(end_idx) = content.find(end_marker) {
                blocks.push(&cursor[..start_idx + start_marker.len() + 1 + end_idx]);
                cursor = &content[end_idx + end_marker.len()..];
            } else {
                return blocks;
            }
        } else {
            cursor = after_start;
        }
    }
}

fn last_heading(src: &str, prefix: &str) -> Option<String> {
    src.lines()
        .rev()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim).map(str::to_string))
}

fn heading_field_marker(key: &str) -> String {
    format!("<!-- WW:HEADING {key} -->")
}

fn field_start_prefix(key: &str) -> String {
    format!("<!-- WW:FIELD {key}")
}

fn block_field_start_marker(key: &str) -> String {
    format!("<!-- WW:FIELD {key} -->")
}

fn field_end_marker(key: &str) -> String {
    format!("<!-- /WW:FIELD {key} -->")
}

fn parse_inline_field_header(header: &str, key: &str) -> Option<String> {
    let prefix = format!("<!-- WW:FIELD {key} = ");
    let suffix = " -->";
    header
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_markdown_roundtrip() {
        let world = WorldDescription {
            name: "Cyber Runner".into(),
            main_description: "Intro\n# heading inside content\n## another one".into(),
            pc_descriptions: BTreeMap::from([
                (
                    "Runner".into(),
                    PcDescription {
                        description: "desc\n# inner heading".into(),
                        initial_action: "go".into(),
                    },
                ),
                (
                    "Fixer".into(),
                    PcDescription {
                        description: "other desc".into(),
                        initial_action: "wait\n".into(),
                    },
                ),
            ]),
            init_action: "start\nwith newline".into(),
        };

        let markdown = world_to_markdown(&world);
        let parsed = world_from_markdown(&markdown).unwrap();

        assert_eq!(parsed.name, world.name);
        assert_eq!(parsed.main_description, world.main_description);
        assert_eq!(parsed.init_action, world.init_action);
        assert_eq!(parsed.pc_descriptions.len(), world.pc_descriptions.len());

        for (name, expected) in &world.pc_descriptions {
            let actual = parsed.pc_descriptions.get(name).unwrap();
            assert_eq!(actual.description, expected.description);
            assert_eq!(actual.initial_action, expected.initial_action);
        }
    }

    #[test]
    fn serializer_uses_heading_markers_for_names() {
        let world = WorldDescription {
            name: "Cyber Runner".into(),
            main_description: "multi\nline".into(),
            pc_descriptions: BTreeMap::from([(
                "Runner".into(),
                PcDescription {
                    description: "desc".into(),
                    initial_action: "Start".into(),
                },
            )]),
            init_action: "Start".into(),
        };

        let markdown = world_to_markdown(&world);

        assert!(markdown.contains("<!-- WW:FORMAT 1 -->"));
        assert!(markdown.contains("\n# Cyber Runner\n"));
        assert!(markdown.contains("<!-- WW:HEADING world.name -->"));
        assert!(markdown.contains("\n## Runner\n"));
        assert!(markdown.contains("<!-- WW:HEADING character.name -->"));
        assert!(markdown.contains("<!-- WW:FIELD world.initial_action -->"));
        assert!(markdown.contains("<!-- WW:FIELD world.description -->"));
    }

    #[test]
    fn parser_defaults_missing_fields_to_empty() {
        let parsed = world_from_markdown(
            r#"
# Loose world
<!-- WW:HEADING world.name -->

# Characters

## Runner
<!-- WW:HEADING character.name -->
<!-- WW:CHARACTER -->
<!-- /WW:CHARACTER -->
"#,
        )
        .unwrap();

        assert_eq!(parsed.name, "Loose world");
        assert_eq!(parsed.main_description, "");
        assert_eq!(parsed.init_action, "");
        assert_eq!(parsed.pc_descriptions["Runner"].description, "");
        assert_eq!(parsed.pc_descriptions["Runner"].initial_action, "");
    }

    #[test]
    fn parser_finds_fields_out_of_order() {
        let parsed = world_from_markdown(
            r#"
<!-- WW:FORMAT 1 -->

<!-- WW:FIELD world.initial_action -->
Begin
<!-- /WW:FIELD world.initial_action -->

# Characters

## Runner
<!-- WW:HEADING character.name -->
<!-- WW:CHARACTER -->

### Description

<!-- WW:FIELD character.description -->
Sneaky
<!-- /WW:FIELD character.description -->
<!-- /WW:CHARACTER -->

# Loose world
<!-- WW:HEADING world.name -->
"#,
        )
        .unwrap();

        assert_eq!(parsed.name, "Loose world");
        assert_eq!(parsed.init_action, "Begin");
        assert_eq!(parsed.pc_descriptions["Runner"].description, "Sneaky");
    }

    #[test]
    fn parser_does_not_shift_fields_between_character_sections() {
        let parsed = world_from_markdown(
            r#"
# Characters test
<!-- WW:HEADING world.name -->

# Characters

## Runner
<!-- WW:HEADING character.name -->
<!-- WW:CHARACTER -->
<!-- /WW:CHARACTER -->

## Fixer
<!-- WW:HEADING character.name -->
<!-- WW:CHARACTER -->

<!-- WW:FIELD character.description -->
Sneaky
<!-- /WW:FIELD character.description -->
<!-- /WW:CHARACTER -->
"#,
        )
        .unwrap();

        assert_eq!(parsed.pc_descriptions["Runner"].description, "");
        assert_eq!(parsed.pc_descriptions["Fixer"].description, "Sneaky");
    }
}
