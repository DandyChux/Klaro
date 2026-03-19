//! Named Entity Recognition module using Candle with DistilBERT-NER
//!
//! This module provides NER capabilities for detecting person names, locations,
//! and organizations in text - entities that are difficult to detect with regex.
//!
//! Model: elastic/distilbert-base-cased-finetuned-conll03-english (~261 MB)

use candle_core::{DType, Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::distilbert::{Config as DistilBertConfig, DistilBertModel};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokenizers::Tokenizer;

// ============================================================================
// NER Labels (CoNLL-2003 format)
// ============================================================================

/// NER entity labels from the model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NerLabel {
    Outside,    // O - not an entity
    BeginPer,   // B-PER - beginning of person name
    InsidePer,  // I-PER - inside person name
    BeginOrg,   // B-ORG - beginning of organization
    InsideOrg,  // I-ORG - inside organization
    BeginLoc,   // B-LOC - beginning of location
    InsideLoc,  // I-LOC - inside location
    BeginMisc,  // B-MISC - beginning of miscellaneous
    InsideMisc, // I-MISC - inside miscellaneous
}

impl NerLabel {
    /// Map label ID to NerLabel (based on model's id2label)
    /// Based on elastic/distilbert-base-cased-finetuned-conll03-english id2label:
    /// 0: O, 1: B-PER, 2: I-PER, 3: B-ORG, 4: I-ORG, 5: B-LOC, 6: I-LOC, 7: B-MISC, 8: I-MISC
    fn from_id(id: usize) -> Self {
        match id {
            0 => NerLabel::Outside,
            1 => NerLabel::BeginPer,
            2 => NerLabel::InsidePer,
            3 => NerLabel::BeginOrg,
            4 => NerLabel::InsideOrg,
            5 => NerLabel::BeginLoc,
            6 => NerLabel::InsideLoc,
            7 => NerLabel::BeginMisc,
            8 => NerLabel::InsideMisc,
            _ => NerLabel::Outside,
        }
    }

    /// Returns the high-level entity type
    pub fn entity_type(&self) -> Option<NerEntityType> {
        match self {
            NerLabel::BeginPer | NerLabel::InsidePer => Some(NerEntityType::Person),
            NerLabel::BeginOrg | NerLabel::InsideOrg => Some(NerEntityType::Organization),
            NerLabel::BeginLoc | NerLabel::InsideLoc => Some(NerEntityType::Location),
            NerLabel::BeginMisc | NerLabel::InsideMisc => Some(NerEntityType::Misc),
            NerLabel::Outside => None,
        }
    }

    /// Check if this is a "begin" label
    pub fn is_begin(&self) -> bool {
        matches!(
            self,
            NerLabel::BeginPer | NerLabel::BeginOrg | NerLabel::BeginLoc | NerLabel::BeginMisc
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NerEntityType {
    Person,
    Organization,
    Location,
    Misc,
}

/// A detected named entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerEntity {
    pub entity_type: NerEntityType,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
}

// ============================================================================
// NER Model
// ============================================================================

/// The NER model wrapper
pub struct NerModel {
    model: DistilBertModel,
    classifier: candle_nn::Linear,
    tokenizer: Tokenizer,
    device: Device,
    num_labels: usize,
}

impl NerModel {
    /// Load model from bundled resources
    pub fn load(app_handle: &tauri::AppHandle) -> Result<Self, String> {
        use tauri::Manager;

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        let device = Device::Cpu;

        // Resolve resource paths
        let resource_dir = app_handle
            .path()
            .resolve("resources/models/ner", tauri::path::BaseDirectory::Resource)
            .map_err(|e| format!("Failed to resolve resource path: {}", e))?;

        let config_path = resource_dir.join("config.json");
        let weights_path = resource_dir.join("model.safetensors");
        let tokenizer_path = resource_dir.join("tokenizer.json");

        // Verify files exist
        for (name, path) in [
            ("config", &config_path),
            ("weights", &weights_path),
            ("tokenizer", &tokenizer_path),
        ] {
            if !path.exists() {
                return Err(format!("{} file not found at {:?}", name, path));
            }
        }

        println!("Loading model from {:?}", resource_dir);

        // Load and parse config
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        // Parse as serde_json::Value first to handle potential duplicate keys
        let config_value: serde_json::Value = serde_json::from_str(&config_str)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;

        let num_labels = config_value
            .get("id2label")
            .and_then(|v| v.as_object())
            .map(|obj| obj.len())
            .unwrap_or(9);

        let model_config: DistilBertConfig = serde_json::from_value(config_value)
            .map_err(|e| format!("Failed to parse DistilBertConfig: {}", e))?;

        // Load weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)
                .map_err(|e| format!("Failed to load weights: {}", e))?
        };

        // Load DistilBERT model
        let model = DistilBertModel::load(vb.pp("distilbert"), &model_config)
            .map_err(|e| format!("Failed to load DistilBERT model: {}", e))?;

        // Load classifier head (token classification)
        let classifier = candle_nn::linear(model_config.dim, num_labels, vb.pp("classifier"))
            .map_err(|e| format!("Failed to load classifier: {}", e))?;

        // Load tokenizer
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        // Configure padding & truncation for batch processing
        tokenizer.with_padding(Some(tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::BatchLongest,
            direction: tokenizers::PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id: 0,
            pad_type_id: 0,
            pad_token: String::from("[PAD]"),
        }));
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                strategy: tokenizers::TruncationStrategy::LongestFirst,
                stride: 0,
                ..Default::default()
            }))
            .map_err(|e| format!("Failed to set truncation: {}", e))?;

        println!(
            "NER model loaded successfully (dim: {}, labels: {})",
            model_config.dim, num_labels
        );

        Ok(Self {
            model,
            classifier,
            tokenizer,
            device,
            num_labels,
        })
    }

    /// Predict named entities in text
    pub fn predict(&self, texts: &[&str]) -> Result<Vec<Vec<NerEntity>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Filter out empty texts, tracking original indices
        let mut non_empty: Vec<(usize, &str)> = Vec::new();
        let mut results: Vec<Vec<NerEntity>> = vec![Vec::new(); texts.len()];

        for (i, text) in texts.iter().enumerate() {
            if !text.trim().is_empty() {
                non_empty.push((i, text));
            }
        }

        if non_empty.is_empty() {
            return Ok(results);
        }

        // Batch-encode all non-empty texts at once (padding applied automatically)
        let texts_to_encode: Vec<&str> = non_empty.iter().map(|(_, text)| *text).collect();
        let encodings = self
            .tokenizer
            .encode_batch(texts_to_encode, true)
            .map_err(|e| format!("Batch tokenization failed: {}", e))?;

        let batch_size = encodings.len();
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(512))
            .max()
            .unwrap_or(0);

        if max_len == 0 {
            return Ok(results);
        }

        // Flatten into contiguous buffers for tensor creation
        let mut all_input_ids: Vec<u32> = Vec::with_capacity(batch_size * max_len);
        let mut all_attention_mask: Vec<f32> = Vec::with_capacity(batch_size * max_len);

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(max_len);

            all_input_ids.extend_from_slice(&ids[..len]);
            all_attention_mask.extend(mask[..len].iter().map(|&x| x as f32));

            // Pad to max len
            let pad = max_len - len;
            all_input_ids.extend(std::iter::repeat(0u32).take(pad));
            all_attention_mask.extend(std::iter::repeat(0f32).take(pad));
        }

        // Build 2D tensors: [batch_size, max_len]
        let input_ids_tensor =
            Tensor::from_slice(&all_input_ids, (batch_size, max_len), &self.device)
                .map_err(|e| format!("Failed to create batch input tensor: {}", e))?;

        let attention_mask_tensor =
            Tensor::from_slice(&all_attention_mask, (batch_size, max_len), &self.device)
                .map_err(|e| format!("Failed to create batch mask tensor: {}", e))?;

        // Wrap forward pass in autoreleasepool to prevent Metal memory leak
        // (see: https://github.com/huggingface/candle/issues/2271)
        #[cfg(target_os = "macos")]
        let (all_predictions, all_probs) = objc::rc::autoreleasepool(|| {
            self.forward_pass(&input_ids_tensor, &attention_mask_tensor)
        })?;

        // On other platforms, just run directly.
        #[cfg(not(target_os = "macos"))]
        let (all_predictions, all_probs) =
            self.forward_pass(&input_ids_tensor, &attention_mask_tensor)?;

        for (batch_idx, (orig_idx, text)) in non_empty.iter().enumerate() {
            let predictions: Vec<u32> = all_predictions
                .get(batch_idx)
                .map_err(|e| format!("Failed to get predictions for item {}: {}", batch_idx, e))?
                .to_vec1()
                .map_err(|e| format!("Failed to convert predictions: {}", e))?;

            let probs = all_probs
                .get(batch_idx)
                .map_err(|e| format!("Failed to get probs for item {}: {}", batch_idx, e))?;

            let offsets = encodings[batch_idx].get_offsets();
            let seq_len = encodings[batch_idx].get_ids().len().min(max_len);

            let entities =
                self.extract_entities(text, &predictions[..seq_len], &probs, &offsets[..seq_len])?;
            results[*orig_idx] = entities;
        }

        // Force cleanup of intermediate tensors to prevent memory buildup
        // across hundreds of batched forward passes
        let _ = self.device.synchronize();

        Ok(results)
    }

    /// Run the model forward pass and return predictions + probabilities.
    /// Extracted so it can be wrapped in autoreleasepool on macOS.
    fn forward_pass(
        &self,
        input_ids: &Tensor,
        attention_mask: &Tensor,
    ) -> Result<(Tensor, Tensor), String> {
        let hidden_states = self
            .model
            .forward(input_ids, attention_mask)
            .map_err(|e| format!("Batch forward pass failed: {}", e))?;

        let logits = self
            .classifier
            .forward(&hidden_states)
            .map_err(|e| format!("Batch classifier failed: {}", e))?;

        let all_predictions = logits
            .argmax(2)
            .map_err(|e| format!("Batch argmax failed: {}", e))?;

        let all_probs = candle_nn::ops::softmax(&logits, 2)
            .map_err(|e| format!("Batch softmax failed: {}", e))?;

        Ok((all_predictions, all_probs))
    }

    fn extract_entities(
        &self,
        text: &str,
        predictions: &[u32],
        probs: &Tensor,
        offsets: &[(usize, usize)],
    ) -> Result<Vec<NerEntity>, String> {
        let mut entities = Vec::new();
        let mut current: Option<(NerEntityType, usize, usize, f32)> = None;

        for (i, &pred_id) in predictions.iter().enumerate() {
            let label = NerLabel::from_id(pred_id as usize);
            let (start, end) = offsets[i];

            // Skip special tokens
            if start == 0 && end == 0 && i > 0 {
                continue;
            }

            let confidence: f32 = probs
                .get(i)
                .and_then(|row| row.get(pred_id as usize))
                .and_then(|t| t.to_scalar())
                .unwrap_or(0.0);

            match label.entity_type() {
                Some(entity_type) => {
                    if label.is_begin() || current.as_ref().map(|e| e.0) != Some(entity_type) {
                        // Save previous entity
                        if let Some((etype, estart, eend, conf)) = current.take() {
                            if eend <= text.len() {
                                entities.push(NerEntity {
                                    entity_type: etype,
                                    text: text[estart..eend].to_string(),
                                    start: estart,
                                    end: eend,
                                    confidence: conf,
                                });
                            }
                        }
                        current = Some((entity_type, start, end, confidence));
                    } else if let Some((_, _, ref mut eend, ref mut conf)) = current {
                        *eend = end;
                        *conf = (*conf + confidence) / 2.0;
                    }
                }
                None => {
                    if let Some((etype, estart, eend, conf)) = current.take() {
                        if eend <= text.len() {
                            entities.push(NerEntity {
                                entity_type: etype,
                                text: text[estart..eend].to_string(),
                                start: estart,
                                end: eend,
                                confidence: conf,
                            });
                        }
                    }
                }
            }
        }

        // Final entity
        if let Some((entity_type, start, end, confidence)) = current {
            if end <= text.len() {
                entities.push(NerEntity {
                    entity_type: entity_type,
                    text: text[start..end].to_string(),
                    start: start,
                    end: end,
                    confidence: confidence,
                });
            }
        }

        Ok(entities)
    }
}

// ============================================================================
// Global NER Model
// ============================================================================

static NER_MODEL: OnceLock<Result<NerModel, String>> = OnceLock::new();

/// Initialize NER (call once during app startup)
pub fn initialize_ner(app_handle: &tauri::AppHandle) -> Result<&'static NerModel, String> {
    NER_MODEL
        .get_or_init(|| NerModel::load(app_handle))
        .as_ref()
        .map_err(|e| e.clone())
}

/// Check if NER is available
pub fn is_ner_available() -> bool {
    NER_MODEL.get().is_some_and(|r| r.is_ok())
}

/// Predict entities using the global NER model
pub fn predict_entities(texts: &[&str]) -> Result<Vec<Vec<NerEntity>>, String> {
    match NER_MODEL.get() {
        Some(Ok(model)) => model.predict(texts),
        Some(Err(e)) => Err(e.clone()),
        None => Err("NER not initialized".to_string()),
    }
}
