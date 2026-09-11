// SPDX-License-Identifier: MIT
//! Paragraph and code-block splitting primitives.

pub(crate) fn split_paragraphs(lines: &[String]) -> Vec<Vec<String>> {
    let mut paras: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paras.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line.clone());
        }
    }
    if !current.is_empty() {
        paras.push(current);
    }
    paras
}

pub(crate) fn is_code_fence_paragraph(para: &[String]) -> bool {
    let first = para.first().map(|s| s.trim_start()).unwrap_or("");
    let last = para.last().map(|s| s.trim_start()).unwrap_or("");
    first.starts_with("```") && last.starts_with("```") && para.len() > 1
}

pub(crate) fn split_long_paragraph(para: &[String], budget: usize) -> Vec<Vec<String>> {
    para.chunks(budget).map(|c| c.to_vec()).collect()
}

pub(crate) fn split_code_block(para: &[String], budget: usize) -> Vec<Vec<String>> {
    if para.len() <= budget {
        return vec![para.to_vec()];
    }

    let Some(opener) = para.first().cloned() else {
        return Vec::new();
    };
    let Some(closer) = para.last().cloned() else {
        return Vec::new();
    };

    let content = &para[1..para.len() - 1];
    let mut chunks: Vec<Vec<String>> = Vec::new();
    for content_chunk in content.chunks(budget.saturating_sub(2).max(1)) {
        let mut block = vec![opener.clone()];
        block.extend(content_chunk.iter().cloned());
        block.push(closer.clone());
        chunks.push(block);
    }
    chunks
}
