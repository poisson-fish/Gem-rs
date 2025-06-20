/// This module contains constants and types related to the Gemini API.
///
/// Gem-rs is a Rust library that serves as a wrapper around the Gemini API,
/// providing support for streaming. This library is designed to facilitate
/// interaction with the Gemini API, making it easier to integrate its
/// functionalities into Rust projects.
use serde::{Deserialize, Serialize};

/// Base URL for generating content using the Gemini API.
pub const GENERATE_CONTENT: &str = "https://generativelanguage.googleapis.com/v1beta/models/";

/// Base URL for streaming content generation using the Gemini API.
pub const STREAM_GENERATE_CONTENT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/";

/// Enum representing different Gemini API models.
///
/// This enum includes various versions of Gemini models, including experimental
/// and stable versions. The default model is set to `Gemini15Pro`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Models {
    /// Experimental Gemini 1.5 Pro model (version 0827)
    #[serde(rename = "gemini-1.5-pro-exp-0827")]
    Gemini15ProExp0827,

    /// Experimental Gemini 1.5 Flash model (version 0827)
    #[serde(rename = "gemini-1.5-flash-exp-0827")]
    Gemini15FlashExp0827,

    /// Experimental Gemini 1.5 Flash 8B model (version 0827)
    #[serde(rename = "gemini-1.5-flash-8b-exp-0827")]
    Gemini15Flash8bExp0827,

    /// Gemini 1.5 Pro model
    #[serde(rename = "gemini-1.5-pro")]
    Gemini15Pro,

    /// Gemini 2 Flash Experimental model
    #[serde(rename = "gemini-2.0-flash-exp")]
    Gemini2FlashExp,

    /// Default Gemini 2 Flash model
    #[default]
    #[serde(rename = "gemini-2.0-flash")]
    Gemini2Flash,

    /// Gemini 2 Flash Lite model
    #[serde(rename = "gemini-2.0-flash-lite")]
    Gemini2FlashLite,

    /// Gemini 2 Flash Thinking Experimental model
    #[serde(rename = "gemini-2.0-flash-thinking-exp-01-21")]
    Gemini2FlashThinkingExp,

    /// Gemini 2 Experimental model
    #[serde(rename = "gemini-exp-1206")]
    Gemini2ProExp1206,

    /// Gemini 2 Experimental model
    #[serde(rename = "gemini-2.0-pro-exp-02-05")]
    Gemini2ProExp,

    /// Gemini 2.5 Experimental model
    #[serde(rename = "gemini-2.5-pro-preview-05-06")]
    Gemini25ProExp,

    /// Gemini 1.5 Flash model
    #[serde(rename = "gemini-1.5-flash")]
    Gemini15Flash,

    /// Gemini 1.0 Pro model
    #[serde(rename = "gemini-1.0-pro")]
    Gemini10Pro,

    /// Gemma 2 2B IT model
    #[serde(rename = "gemma-2-2b-it")]
    Gemma2_2bIt,

    /// Gemma 2 9B IT model
    #[serde(rename = "gemma-2-9b-it")]
    Gemma2_9bIt,

    /// Gemma 2 27B IT model
    #[serde(rename = "gemma-2-27b-it")]
    Gemma2_27bIt,

    /// Custom model
    Custom(String),
}

impl ToString for Models {
    fn to_string(&self) -> String {
        match self {
            Models::Custom(model) => model.to_string().replace("\"", ""),
            _ => serde_json::to_string(self).unwrap().replace("\"", ""),
        }
    }
}
