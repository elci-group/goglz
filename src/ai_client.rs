use crate::config::{Config, GptOssConfig, GroqConfig};
use crate::error::{GoglzError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptualizationResult {
    pub summary: String,
    pub key_concepts: Vec<String>,
    pub themes: Vec<String>,
    pub clarity_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarityImprovement {
    pub original_text: String,
    pub improved_text: String,
    pub changes_made: Vec<String>,
    pub clarity_improvement: f32,
}

pub struct AiClient {
    client: Client,
    gpt_oss_config: GptOssConfig,
    groq_config: GroqConfig,
}

impl AiClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            gpt_oss_config: config.gpt_oss.clone(),
            groq_config: config.groq.clone(),
        }
    }

    pub async fn conceptualize_document(&self, text: &str) -> Result<ConceptualizationResult> {
        let prompt = format!(
            "Analyze the following document and provide:\n\
            1. A concise summary (2-3 sentences)\n\
            2. Key concepts (bullet points)\n\
            3. Main themes (bullet points)\n\
            4. A clarity score from 0.0 to 1.0\n\n\
            Document:\n{}",
            text
        );

        let response = self
            .call_gpt_oss(&self.gpt_oss_config.model_120b, &prompt)
            .await?;

        self.parse_conceptualization(&response)
    }

    pub async fn improve_clarity_with_llama(&self, text: &str) -> Result<ClarityImprovement> {
        let prompt = format!(
            "Improve the clarity of the following text. Return your response as JSON with these fields:\n\
            - \"improved_text\": the improved version\n\
            - \"changes_made\": list of specific changes\n\
            - \"clarity_improvement\": estimated improvement factor (1.0 = no change, >1.0 = improvement)\n\n\
            Original text:\n{}",
            text
        );

        let response = self.call_groq(&prompt).await?;
        self.parse_clarity_improvement(text, &response)
    }

    pub async fn revise_document(&self, prompt: &str) -> Result<String> {
        let response = self.call_groq(prompt).await?;
        
        // Clean up response - remove any markdown code blocks if present
        let cleaned = response
            .trim_start_matches("```")
            .trim_start_matches("```markdown")
            .trim_start_matches("```text")
            .trim_end_matches("```")
            .trim();
        
        Ok(cleaned.to_string())
    }

    async fn call_gpt_oss(&self, model: &str, prompt: &str) -> Result<String> {
        let request_body = json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert at analyzing documents and improving their clarity. Always respond with well-structured, parseable output."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 2000,
            "temperature": 0.3
        });

        let response = self
            .client
            .post(&format!("{}/chat/completions", self.gpt_oss_config.api_endpoint))
            .header("Authorization", format!("Bearer {}", self.gpt_oss_config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(GoglzError::ProcessingFailed(format!(
                "GPT OSS API error: {}",
                error_text
            )));
        }

        let response_json: serde_json::Value = response.json().await?;
        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| GoglzError::ProcessingFailed("Invalid API response format".to_string()))
    }

    async fn call_groq(&self, prompt: &str) -> Result<String> {
        let request_body = json!({
            "model": self.groq_config.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert at improving text clarity. Always respond with valid JSON."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 2000,
            "temperature": 0.3
        });

        let response = self
            .client
            .post(&format!("{}/chat/completions", self.groq_config.api_endpoint))
            .header("Authorization", format!("Bearer {}", self.groq_config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(GoglzError::ProcessingFailed(format!(
                "Groq API error: {}",
                error_text
            )));
        }

        let response_json: serde_json::Value = response.json().await?;
        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| GoglzError::ProcessingFailed("Invalid API response format".to_string()))
    }

    fn parse_conceptualization(&self, response: &str) -> Result<ConceptualizationResult> {
        // Try to parse as JSON first
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
            return Ok(ConceptualizationResult {
                summary: json["summary"]
                    .as_str()
                    .unwrap_or("No summary available")
                    .to_string(),
                key_concepts: json["key_concepts"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                themes: json["themes"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                clarity_score: json["clarity_score"]
                    .as_f64()
                    .unwrap_or(0.5) as f32,
            });
        }

        // Fallback: parse text-based response
        let lines: Vec<&str> = response.lines().collect();
        let mut summary = String::new();
        let mut key_concepts = Vec::new();
        let mut themes = Vec::new();
        let mut clarity_score = 0.5;

        let mut current_section = "";
        for line in lines {
            if line.to_lowercase().contains("summary") {
                current_section = "summary";
                continue;
            } else if line.to_lowercase().contains("key concepts") {
                current_section = "concepts";
                continue;
            } else if line.to_lowercase().contains("themes") {
                current_section = "themes";
                continue;
            } else if line.to_lowercase().contains("clarity") {
                if let Some(score_str) = line.split(':').nth(1) {
                    clarity_score = score_str.trim().parse().unwrap_or(0.5);
                }
                continue;
            }

            match current_section {
                "summary" => summary.push_str(line),
                "concepts" if !line.trim().is_empty() => key_concepts.push(line.trim().to_string()),
                "themes" if !line.trim().is_empty() => themes.push(line.trim().to_string()),
                _ => {}
            }
        }

        Ok(ConceptualizationResult {
            summary: summary.trim().to_string(),
            key_concepts,
            themes,
            clarity_score,
        })
    }

    fn parse_clarity_improvement(&self, original: &str, response: &str) -> Result<ClarityImprovement> {
        // Try to extract JSON from response
        let json_str = response
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            return Ok(ClarityImprovement {
                original_text: original.to_string(),
                improved_text: json["improved_text"]
                    .as_str()
                    .unwrap_or(original)
                    .to_string(),
                changes_made: json["changes_made"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                clarity_improvement: json["clarity_improvement"]
                    .as_f64()
                    .unwrap_or(1.0) as f32,
            });
        }

        // Fallback
        Ok(ClarityImprovement {
            original_text: original.to_string(),
            improved_text: response.to_string(),
            changes_made: vec!["Full text replacement".to_string()],
            clarity_improvement: 1.0,
        })
    }
}
