// SPDX-License-Identifier: MIT
//! Prompt construction for `goglz revise`.

use crate::config::ReviseConfig;

/// Build the full prompt sent to the AI revision endpoint.
pub(crate) fn build_revision_prompt(
    config: &ReviseConfig,
    content: &str,
    asset_context: &str,
) -> String {
    let mut prompt = String::new();

    // Add purpose and scope
    prompt.push_str(&format!("Purpose: {}\n", config.purpose));
    prompt.push_str(&format!("Scope: {}\n\n", config.scope));

    // Add writing style guidelines
    prompt.push_str("Writing Style:\n");
    prompt.push_str(&format!("- Tone: {}\n", config.writing_style.tone));
    prompt.push_str(&format!("- Voice: {}\n", config.writing_style.voice));
    prompt.push_str(&format!("- Audience: {}\n", config.writing_style.audience));
    prompt.push_str("Guidelines:\n");
    for guideline in &config.writing_style.guidelines {
        prompt.push_str(&format!("  - {}\n", guideline));
    }
    prompt.push('\n');

    // Add formatting rules
    prompt.push_str("Formatting Rules:\n");
    if config.formatting_rules.headings {
        prompt.push_str("  - Use clear, hierarchical headings\n");
    }
    if config.formatting_rules.bullet_points {
        prompt.push_str("  - Use bullet points for lists\n");
    }
    if config.formatting_rules.numbered_lists {
        prompt.push_str("  - Use numbered lists for sequential items\n");
    }
    if config.formatting_rules.code_blocks {
        prompt.push_str("  - Use code blocks for technical content\n");
    }
    if let Some(max_len) = config.formatting_rules.max_line_length {
        prompt.push_str(&format!(
            "  - Maximum line length: {} characters\n",
            max_len
        ));
    }
    for custom_rule in &config.formatting_rules.custom_rules {
        prompt.push_str(&format!("  - {}\n", custom_rule));
    }
    prompt.push('\n');

    // Add asset context if available
    if !asset_context.is_empty() {
        prompt.push_str("Reference Assets:\n");
        prompt.push_str(asset_context);
        prompt.push('\n');
    }

    // Add the document content
    prompt.push_str("Original Document:\n");
    prompt.push_str(content);
    prompt.push_str("\n\n");

    // Add instruction
    prompt.push_str("Please revise the document according to the above guidelines. ");
    prompt.push_str(
        "Return only the revised document content without any additional commentary or metadata.",
    );

    prompt
}
