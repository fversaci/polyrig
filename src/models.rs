/**************************************************************************
  Copyright 2026 Francesco Versaci (https://github.com/fversaci/)

  This program is free software: you can redistribute it and/or modify
  it under the terms of the GNU Affero General Public License as published by
  the Free Software Foundation, either version 3 of the License, or
  (at your option) any later version.

  This program is distributed in the hope that it will be useful,
  but WITHOUT ANY WARRANTY; without even the implied warranty of
  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
  GNU Affero General Public License for more details.

  You should have received a copy of the GNU Affero General Public License
  along with this program.  If not, see <https://www.gnu.org/licenses/>.
**************************************************************************/
use anyhow::{Context, Result};
use serde::Deserialize;

const MODELS_URL: &str = "https://openrouter.ai/api/v1/models?output_modalities=speech";

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    supported_voices: Option<Vec<String>>,
}

/// Returns the list of OpenRouter models that output speech (TTS models).
async fn speech_models() -> Result<Vec<ModelEntry>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(MODELS_URL)
        .send()
        .await
        .context("Failed to reach the OpenRouter models API")?;
    if !resp.status().is_success() {
        anyhow::bail!("OpenRouter models API returned status {}", resp.status());
    }
    let parsed: ModelsResponse = resp
        .json()
        .await
        .context("Failed to parse the OpenRouter models API response")?;
    Ok(parsed.data)
}

/// Returns the voice identifiers supported by the given TTS model.
///
/// Returns an empty list when the model exists but publishes no
/// `supported_voices` (e.g. voice-cloning models).
pub async fn supported_tts_voices(model: &str) -> Result<Vec<String>> {
    let models = speech_models().await?;
    models
        .into_iter()
        .find(|m| m.id == model)
        .map(|m| m.supported_voices.unwrap_or_default())
        .ok_or_else(|| anyhow::anyhow!("Model '{}' not found among TTS models", model))
}

/// Returns the ids of all OpenRouter models that output speech.
pub async fn tts_model_ids() -> Result<Vec<String>> {
    Ok(speech_models().await?.into_iter().map(|m| m.id).collect())
}
