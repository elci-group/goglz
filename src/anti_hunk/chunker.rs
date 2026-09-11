// SPDX-License-Identifier: MIT
//! Paragraph-level chunking for oversized markdown sections.

use crate::anti_hunk::split;

/// Split a section body (already trimmed of surrounding blank lines) into
/// chunks where each chunk's paragraph lines fit inside `budget`. Paragraphs
/// are preserved whole when possible. Code blocks are kept as complete fenced
/// blocks even when they must be subdivided.
pub(crate) fn chunk_body(body: &[String], budget: usize) -> Vec<Vec<Vec<String>>> {
    let paras = split::split_paragraphs(body);
    let mut chunks: Vec<Vec<Vec<String>>> = Vec::new();
    let mut current: Vec<Vec<String>> = Vec::new();
    let mut current_len: usize = 0;

    for para in paras {
        if para.is_empty() {
            continue;
        }

        if split::is_code_fence_paragraph(&para) && para.len() > budget {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            for sub in split::split_code_block(&para, budget) {
                chunks.push(vec![sub]);
            }
            continue;
        }

        if para.len() > budget {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            for sub in split::split_long_paragraph(&para, budget) {
                chunks.push(vec![sub]);
            }
            continue;
        }

        let added = if current.is_empty() {
            para.len()
        } else {
            para.len() + 1
        };

        if current_len + added > budget {
            chunks.push(std::mem::take(&mut current));
            current = vec![para.clone()];
            current_len = para.len();
        } else {
            current.push(para.clone());
            current_len += added;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}
