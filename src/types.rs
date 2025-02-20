use std::{collections::HashMap, path::Path};

use base64::{engine::general_purpose, Engine as _};
use dotenv::dotenv;
use log::log;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::{errors::GemError, utils::get_mime_type};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")] // Untagged for different types
pub enum PartData {
    InlineData { inline_data: Blob },
    FileData { file_data: FileData },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Model,
    #[default]
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // Ensure enum variants match the JSON casing
pub(crate) enum FinishReason {
    FinishReasonUnspecified, // Default value. This value is unused.
    Stop,                    // Natural stop point of the model or provided stop sequence.
    MaxTokens,  // The maximum number of tokens as specified in the request was reached.
    Safety,     // The response candidate content was flagged for safety reasons.
    Recitation, // The response candidate content was flagged for recitation reasons.
    Language,   // The response candidate content was flagged for using an unsupported language.
    Other,      // Unknown reason.
    Blocklist,  // Token generation stopped because the content contains forbidden terms.
    ProhibitedContent, // Token generation stopped for potentially containing prohibited content.
    Spii, // Token generation stopped because the content potentially contains Sensitive Personally Identifiable Information (SPII).
    MalformedFunctionCall, // The function call generated by the model is invalid.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    candidates: Vec<Candidate>,
    prompt_feedback: Option<PromptFeedback>, // This is optional
    usage_metadata: Option<UsageMetadata>,   // This is optional
}

impl GenerateContentResponse {
    pub fn get_candidates(&self) -> &Vec<Candidate> {
        &self.candidates
    }

    pub fn get_results(&self) -> Vec<String> {
        let mut texts = Vec::new();
        for candidate in &self.candidates {
            if let Some(content) = candidate.get_content() {
                if let Some(text) = content.get_text() {
                    texts.push(text.clone());
                }
            }
        }
        texts
    }

    pub fn get_usage_metadata(&self) -> Option<&UsageMetadata> {
        self.usage_metadata.as_ref()
    }

    pub(crate) fn feedback(&self) -> Option<BlockReason> {
        match self.prompt_feedback.is_some()
            && self
                .prompt_feedback
                .as_ref()
                .unwrap()
                .block_reason
                .is_some()
        {
            true => self.prompt_feedback.as_ref().unwrap().block_reason.clone(),
            false => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    content: Option<Content>,            // The content generated by the model
    finish_reason: Option<FinishReason>, // Enum to represent why the model stopped
    safety_ratings: Option<Vec<SafetyRating>>, // List of safety ratings for the response
    token_count: Option<i32>,            // The token count for this candidate
    index: Option<i32>,                  // Index of the candidate in the list
}

impl Candidate {
    pub(crate) fn get_content(&self) -> Option<&Content> {
        self.content.as_ref()
    }

    pub(crate) fn is_blocked(&self) -> bool {
        (self.finish_reason == Some(FinishReason::Safety))
            || (self.finish_reason == Some(FinishReason::Recitation))
            || (self.finish_reason == Some(FinishReason::ProhibitedContent))
    }

    pub(crate) fn get_token_count(&self) -> Option<i32> {
        self.token_count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    parts: Vec<Part>,   // A vector of Part objects
    role: Option<Role>, // Role field, optional; either 'user' or 'model'
}

impl Content {
    pub fn get_text(&self) -> Option<String> {
        for part in &self.parts {
            match &part.data {
                PartData::Text { text } => return Some(text.clone()),
                _ => continue,
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoRoleContent {
    parts: Vec<Part>, // A vector of Part objects
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    #[serde(flatten)] // This enables the union-like behavior for the different possible types
    data: PartData, // Union field that can be one of several types
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    mime_type: String,
    data: String, // Base64 encoded data
}

impl Blob {
    pub fn new(mime_type: &str, data: &[u8]) -> Self {
        Blob {
            mime_type: mime_type.to_string(),
            data: general_purpose::STANDARD.encode(&data),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    mime_type: String,
    file_uri: String, // File URI
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptFeedback {
    block_reason: Option<BlockReason>, // Block reason, optional
    safety_ratings: Vec<SafetyRating>, // A vector of SafetyRating objects
}

impl PromptFeedback {
    pub(crate) fn get_block_reason(&self) -> Option<BlockReason> {
        self.block_reason.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // Ensure enum variants match the JSON casing
pub(crate) enum BlockReason {
    BlockReasonUnspecified, // Default value, unused
    Safety,                 // Blocked for safety reasons
    Other,                  // Blocked for unknown reasons
    Blocklist,              // Blocked due to blacklist terms
    ProhibitedContent,      // Blocked due to prohibited content
}

impl std::fmt::Display for BlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockReason::BlockReasonUnspecified => write!(f, "Unspecified"),
            BlockReason::Safety => write!(f, "Safety"),
            BlockReason::Other => write!(f, "Other"),
            BlockReason::Blocklist => write!(f, "Blocklist"),
            BlockReason::ProhibitedContent => write!(f, "Prohibited Content"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SafetyRating {
    category: Option<String>,    // The safety category
    probability: Option<String>, // The probability of the content being unsafe
    blocked: Option<bool>,       // Whether the content is blocked
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    prompt_token_count: Option<i32>, // Number of tokens in the prompt
    cached_content_token_count: Option<i32>, // Number of tokens in cached content
    candidates_token_count: Option<i32>, // Number of tokens in the generated candidates
    total_token_count: Option<i32>,  // Total number of tokens (prompt + candidates)
}

impl UsageMetadata {
    pub fn get_prompt_token_count(&self) -> Option<i32> {
        self.prompt_token_count
    }

    pub fn get_cached_content_token_count(&self) -> Option<i32> {
        self.cached_content_token_count
    }

    pub fn get_candidates_token_count(&self) -> Option<i32> {
        self.candidates_token_count
    }

    pub fn get_total_token_count(&self) -> Option<i32> {
        self.total_token_count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoMetadata {
    video_duration: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    name: String,
    uri: String,
    display_name: String,
    mime_type: String,
    size_bytes: String,
    create_time: String,
    update_time: String,
    expiration_time: String,
    sha256_hash: String,
    state: String,
    error: Option<Status>,
    video_metadata: Option<VideoMetadata>,
    #[serde(skip)]
    api_key: String,
}

impl File {
    pub(crate) async fn new(
        file_name: &str,
        bytes: Vec<u8>,
        mime_type: &str,
        api_key: &str,
    ) -> Result<Self, GemError> {
        Self::upload(file_name, bytes, mime_type, api_key).await
    }

    async fn upload(
        file_name: &str,
        buffer: Vec<u8>,
        mime_type: &str,
        api_key: &str,
    ) -> Result<Self, GemError> {
        let num_bytes = buffer.len();

        let client = reqwest::Client::new();

        let reserve_response = match client
            .post("https://generativelanguage.googleapis.com/upload/v1beta/files")
            .query(&[("key", api_key)])
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", num_bytes.to_string())
            .header("X-Goog-Upload-Header-Content-Type", mime_type)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&json!({
                "file": { "display_name": file_name }
            }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return Err(GemError::FileError(e.to_string())),
        };

        let location = match reserve_response.headers().get("X-Goog-Upload-URL") {
            Some(loc) => match loc.to_str() {
                Ok(l) => l,
                Err(e) => return Err(GemError::FileError(e.to_string())),
            },
            None => {
                return Err(GemError::FileError(
                    "X-Goog-Upload-URL header not found".to_string(),
                ))
            }
        };

        // Uploading the file's bytes
        let upload_response = match client
            .put(location)
            .header("Content-Length", num_bytes.to_string())
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .body(buffer)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return Err(GemError::FileError(e.to_string())),
        };

        let upload_text_response = match upload_response.text().await {
            Ok(t) => t,
            Err(e) => return Err(GemError::FileError(e.to_string())),
        };

        let mut file: File = match serde_json::from_str::<Value>(&upload_text_response) {
            Ok(data) => match data.get("file") {
                Some(f) => match serde_json::from_value(f.clone()) {
                    Ok(file) => file,
                    Err(e) => {
                        log::error!("File error [0]: {} - Response: {}", e, upload_text_response);
                        return Err(GemError::FileError(e.to_string()));
                    }
                },
                None => return Err(GemError::FileError("File data not found".to_string())),
            },
            Err(e) => {
                log::error!("File error [1]: {} - Response: {}", e, upload_text_response);
                return Err(GemError::FileError(e.to_string()));
            }
        };

        // if let Some(name) = file.name.split('/').last() {
        //     file.name = name.to_string();
        // }

        // Check if the file is processed with timeout
        let mut timeout = 0;
        loop {
            let file_state = match client
                .get(&format!(
                    "https://generativelanguage.googleapis.com/v1beta/{}",
                    file.name
                ))
                .query(&[("key", api_key)])
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => return Err(GemError::FileError(e.to_string())),
            };

            let file_state_text_response = match file_state.text().await {
                Ok(t) => t,
                Err(e) => return Err(GemError::FileError(e.to_string())),
            };

            let file_state: File = match serde_json::from_str::<File>(&file_state_text_response) {
                Ok(f) => f,
                Err(e) => {
                    log::error!(
                        "File error [3]: {:#?}, response: {:#?}",
                        e,
                        file_state_text_response
                    );
                    return Err(GemError::FileError("File data not found".to_string()));
                }
            };

            if file_state.state == "ACTIVE" {
                break;
            } else if file_state.state == "FAILED" {
                return Err(GemError::FileError(
                    file_state
                        .error
                        .clone()
                        .unwrap_or(Status {
                            code: 0,
                            message: "File processing failed".to_string(),
                        })
                        .message,
                ));
            } else if file_state.state != "PROCESSING" {
                return Err(GemError::FileError(
                    "File processing unknown state".to_string(),
                ));
            }

            if timeout >= 3 {
                return Err(GemError::FileError("File processing timeout".to_string()));
            }

            timeout += 1;
            std::thread::sleep(std::time::Duration::from_secs(3));
        }

        file.api_key = api_key.to_string();
        Ok(file)
    }

    //TODO: Something with the API cause the cached files in cloud to change uri every time they are deleted
    async fn delete(self) -> Result<(), GemError> {
        log::info!("Deleting file: {:#?}", self);
        if self.api_key == "" {
            log::info!("API key not found: {:#?}", self.display_name);
            return Err(GemError::FileError("API key not found".to_string()));
        }
        let client = reqwest::Client::new();
        match client
            .delete(self.uri)
            .query(&[("key", self.api_key.clone())])
            .send()
            .await
        {
            Ok(_) => {
                log::info!("File deleted successfully: {:#?}", self.display_name);
                Ok(())
            }
            Err(e) => Err(GemError::FileError(e.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct FileManager {
    files: Mutex<HashMap<String, File>>,
    api_key: String,
}

impl FileManager {
    pub fn new() -> Self {
        dotenv().expect("Failed to load Gemini API key");
        let api_key = std::env::var("GEMINI_API_KEY").unwrap();

        Self {
            files: Mutex::new(HashMap::new()),
            api_key: api_key.to_string(),
        }
    }

    pub async fn add_file_from_bytes(
        &self,
        file_name: &str,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<FileData, GemError> {
        let hash = sha256::digest(&bytes);
        match self.get_file(&hash).await {
            Some(file) => Ok(file),
            None => {
                let file = File::new(file_name, bytes, mime_type, &self.api_key).await?;
                let mime_type = file.mime_type.clone();
                let file_uri = file.uri.clone();
                let mut files = self.files.lock().await;
                files.insert(hash, file);
                Ok(FileData {
                    mime_type: mime_type,
                    file_uri: file_uri,
                })
            }
        }
    }

    pub async fn add_file(&mut self, file_path: &Path) -> Result<FileData, GemError> {
        if !file_path.exists() {
            return Err(GemError::FileError("File does not exist".to_string()));
        }

        let file_name = match file_path.file_name() {
            Some(name) if name.to_str().is_some() => name.to_str().unwrap(),
            _ => return Err(GemError::FileError("Invalid file name".to_string())),
        };

        let mut file = match std::fs::File::open(file_path) {
            Ok(f) => f,
            Err(e) => return Err(GemError::FileError(e.to_string())),
        };

        let mut buffer = Vec::new();
        match std::io::Read::read_to_end(&mut file, &mut buffer) {
            Ok(_) => (),
            Err(e) => return Err(GemError::FileError(e.to_string())),
        };

        let mime_type = match get_mime_type(file_path) {
            Some(ext) => ext,
            None => return Err(GemError::FileError("Unsupported file type".to_string())),
        };

        let hash = sha256::digest(&buffer);

        match self.get_file(&hash).await {
            Some(file) => Ok(file),
            None => {
                let file = File::new(file_name, buffer, &mime_type, &self.api_key).await?;
                let mime_type = file.mime_type.clone();
                let file_uri = file.uri.clone();
                let mut files = self.files.lock().await;
                files.insert(hash, file);
                Ok(FileData {
                    mime_type: mime_type,
                    file_uri: file_uri,
                })
            }
        }
    }

    pub async fn check_file(&self, hash: &str) -> bool {
        let files = self.files.lock().await;
        for file in files.iter() {
            if file.0 == hash {
                return true;
            }
        }
        false
    }

    pub async fn get_file(&self, hash: &str) -> Option<FileData> {
        let mut to_remove = Vec::new();
        let mut files = self.files.lock().await;
        for file in files.iter() {
            match file.0 == hash {
                true if file.1.expiration_time
                    > (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339() =>
                {
                    log::info!("Found cached File: {:#?}", file.1);
                    return Some(FileData {
                        mime_type: file.1.mime_type.clone(),
                        file_uri: file.1.uri.clone(),
                    });
                }
                true => {
                    to_remove.push(file.0.clone());
                }
                false => continue,
            }
        }

        for hash in to_remove {
            let file = files.remove(&hash);
            if let Some(file) = file {
                let _ = file.delete().await;
            }
        }

        None
    }

    pub async fn fetch_list(&mut self) -> Result<(), GemError> {
        let client = reqwest::Client::new();
        let mut files = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut request = client.get("https://generativelanguage.googleapis.com/v1beta/files");

            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token), ("key", &self.api_key)]);
            } else {
                request = request.query(&[("key", &self.api_key)]);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => return Err(GemError::FileError(e.to_string())),
            };

            let response_text = match response.text().await {
                Ok(data) => data,
                Err(e) => return Err(GemError::FileError(e.to_string())),
            };

            let response_json: Value = match serde_json::from_str(&response_text) {
                Ok(data) => data,
                Err(e) => {
                    log::error!("File error [6]: {}, response: {}", e, response_text);
                    return Err(GemError::FileError(e.to_string()));
                }
            };

            match response_json.get("files") {
                Some(f) => match serde_json::from_value::<Vec<File>>(f.clone()) {
                    Ok(mut new_files) => files.append(&mut new_files),
                    Err(e) => {
                        log::error!("File error [7]: {}, response: {}", e, response_text);
                        return Err(GemError::FileError(e.to_string()));
                    }
                },
                None => {
                    // Means there are no files, not an error
                    break;
                }
            };

            page_token = response_json
                .get("nextPageToken")
                .and_then(|t| t.as_str().map(String::from));
            if page_token.is_none() {
                break;
            }
        }

        let mut files_map = self.files.lock().await;
        for mut file in files {
            file.api_key = self.api_key.clone();
            log::info!("File: {:#?}", file);
            files_map.insert(file.sha256_hash.clone(), file);
        }

        Ok(())
    }

    pub async fn delete_file(&mut self, hash: &str) -> Result<(), GemError> {
        let mut files = self.files.lock().await;
        let file = files.remove(hash);
        match file {
            Some(file) => file.delete().await,
            None => Ok(()),
        }
    }

    pub async fn clear_files(&mut self) {
        let mut files = self.files.lock().await;
        let keys: Vec<String> = files.keys().cloned().collect();
        for key in keys {
            if let Some(file) = files.remove(&key) {
                let _ = file.delete().await;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    category: HarmCategory,        // Enum for the harm category
    threshold: HarmBlockThreshold, // Enum for the harm block threshold
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // To match the JSON format
enum HarmCategory {
    HarmCategoryHateSpeech,
    HarmCategorySexuallyExplicit,
    HarmCategoryDangerousContent,
    HarmCategoryHarassment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    code: i32,
    message: String,
    status: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error {}: {} ({})", self.code, self.message, self.status)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // To match the JSON format
pub enum HarmBlockThreshold {
    HarmBlockThresholdUnspecified, // Unspecified threshold
    BlockLowAndAbove,              // Block content with NEGIGIBLE and above
    BlockMediumAndAbove,           // Block content with NEGIGIBLE, LOW, and above
    BlockOnlyHigh,                 // Block content with only HIGH harm probability
    BlockNone,                     // All content will be allowed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerationConfig {
    stop_sequences: Option<Vec<String>>, // Optional: Up to 5 stop sequences
    response_mime_type: Option<String>, // Optional: MIME type of the response (e.g., text/plain, application/json)
    max_output_tokens: Option<u32>,     // Optional: Max tokens for the response up to 8192
    temperature: Option<f32>,           // Optional: Controls randomness of the output [0.0, 2.0]
    top_p: Option<f32>, // Optional: Maximum cumulative probability for nucleus sampling
    top_k: Option<u32>, // Optional: Maximum number of tokens to consider for top-k sampling
}

pub struct Settings {
    safety_settings: Option<Vec<SafetySetting>>,
    generation_config: Option<GenerationConfig>,
    system_instruction: Option<String>,
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            safety_settings: None,
            generation_config: None,
            system_instruction: None,
        }
    }

    pub fn set_all_safety_settings(&mut self, threshold: HarmBlockThreshold) {
        self.safety_settings = Some(vec![
            SafetySetting {
                category: HarmCategory::HarmCategoryHateSpeech,
                threshold: threshold.clone(),
            },
            SafetySetting {
                category: HarmCategory::HarmCategorySexuallyExplicit,
                threshold: threshold.clone(),
            },
            SafetySetting {
                category: HarmCategory::HarmCategoryDangerousContent,
                threshold: threshold.clone(),
            },
            SafetySetting {
                category: HarmCategory::HarmCategoryHarassment,
                threshold: threshold.clone(),
            },
        ]);
    }

    pub fn set_advance_settings(
        &mut self,
        stop_sequences: Option<Vec<String>>,
        response_mime_type: Option<String>,
        max_output_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<u32>,
    ) {
        self.generation_config = Some(GenerationConfig {
            stop_sequences: stop_sequences,
            response_mime_type: response_mime_type,
            max_output_tokens: max_output_tokens,
            temperature: temperature,
            top_p: top_p,
            top_k: top_k,
        });
    }

    pub fn set_temperature(&mut self, temperature: f32) {
        match &mut self.generation_config {
            Some(config) => config.temperature = Some(temperature),
            None => {
                self.generation_config = Some(GenerationConfig {
                    stop_sequences: None,
                    response_mime_type: None,
                    max_output_tokens: None,
                    temperature: Some(temperature),
                    top_p: None,
                    top_k: None,
                });
            }
        }
    }

    pub fn set_max_output_tokens(&mut self, max_output_tokens: u32) {
        match &mut self.generation_config {
            Some(config) => config.max_output_tokens = Some(max_output_tokens),
            None => {
                self.generation_config = Some(GenerationConfig {
                    stop_sequences: None,
                    response_mime_type: None,
                    max_output_tokens: Some(max_output_tokens),
                    temperature: None,
                    top_p: None,
                    top_k: None,
                });
            }
        }
    }

    pub fn set_system_instruction(&mut self, instruction: &str) {
        self.system_instruction = Some(instruction.to_string());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerateContentRequest {
    contents: Vec<Content>, // Required: List of content objects (conversation history and latest request)
    safety_settings: Option<Vec<SafetySetting>>, // Optional: Safety settings to block unsafe content
    generation_config: Option<GenerationConfig>, // Optional: Configuration for model generation
    system_instruction: Option<NoRoleContent>,   // Optional: Developer set system instructions
}

impl GenerateContentRequest {
    fn new(
        context: &Context,
        config: Option<GenerationConfig>,
        safety: Option<Vec<SafetySetting>>,
        system_instruction: Option<NoRoleContent>,
    ) -> Self {
        GenerateContentRequest {
            contents: context.contents.clone(),
            safety_settings: match safety {
                Some(s) => Some(s),
                None => Some(vec![
                    SafetySetting {
                        category: HarmCategory::HarmCategoryHateSpeech,
                        threshold: HarmBlockThreshold::BlockNone,
                    },
                    SafetySetting {
                        category: HarmCategory::HarmCategorySexuallyExplicit,
                        threshold: HarmBlockThreshold::BlockNone,
                    },
                    SafetySetting {
                        category: HarmCategory::HarmCategoryDangerousContent,
                        threshold: HarmBlockThreshold::BlockNone,
                    },
                    SafetySetting {
                        category: HarmCategory::HarmCategoryHarassment,
                        threshold: HarmBlockThreshold::BlockNone,
                    },
                ]),
            },
            generation_config: match config {
                Some(c) => Some(c),
                None => Some(GenerationConfig {
                    max_output_tokens: Some(8192),
                    temperature: Some(1.0),
                    response_mime_type: None,
                    stop_sequences: None,
                    top_k: None,
                    top_p: None,
                }),
            },
            system_instruction,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    contents: Vec<Content>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            contents: Vec::new(),
        }
    }

    pub fn push_message(&mut self, role: Option<Role>, content: String) {
        self.contents.push(Content {
            role: role,
            parts: vec![Part {
                data: PartData::Text {
                    text: content.to_string(),
                },
            }],
        });
    }

    pub fn push_file(&mut self, role: Option<Role>, file_data: FileData) {
        self.contents.push(Content {
            role: role,
            parts: vec![Part {
                data: PartData::FileData { file_data },
            }],
        });
    }

    pub fn push_blob(&mut self, role: Option<Role>, blob: Blob) {
        self.contents.push(Content {
            role: role,
            parts: vec![Part {
                data: PartData::InlineData { inline_data: blob },
            }],
        });
    }

    pub fn push_message_with_file(
        &mut self,
        role: Option<Role>,
        content: &str,
        file_data: FileData,
    ) {
        self.contents.push(Content {
            role: role,
            parts: vec![
                Part {
                    data: PartData::Text {
                        text: content.to_string(),
                    },
                },
                Part {
                    data: PartData::FileData { file_data },
                },
            ],
        });
    }

    pub fn push_message_with_blob(&mut self, role: Option<Role>, content: &str, blob: Blob) {
        self.contents.push(Content {
            role: role,
            parts: vec![
                Part {
                    data: PartData::Text {
                        text: content.to_string(),
                    },
                },
                Part {
                    data: PartData::InlineData { inline_data: blob },
                },
            ],
        });
    }

    pub fn build(&self, settings: &Settings) -> GenerateContentRequest {
        GenerateContentRequest::new(
            self,
            settings.generation_config.clone(),
            settings.safety_settings.clone(),
            match &settings.system_instruction {
                Some(instruction) => Some(NoRoleContent {
                    parts: vec![Part {
                        data: PartData::Text {
                            text: instruction.clone(),
                        },
                    }],
                }),
                None => None,
            },
        )
    }

    pub fn clear(&mut self) {
        self.contents.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.contents.len()
    }

    pub fn get_contents(&self) -> &Vec<Content> {
        &self.contents
    }

    pub fn get_contents_mut(&mut self) -> &mut Vec<Content> {
        &mut self.contents
    }
}

mod tests {

    use super::*;

    #[test]
    fn test_deserialize_generate_content_response() {
        let json_data = r#"
        {
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": "Sample text"
                            }
                        ],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "safetyRatings": [
                        {
                            "category": "violence",
                            "probability": "low",
                            "blocked": false
                        }
                    ],
                    "tokenCount": 10,
                    "index": 0
                }
            ],
            "promptFeedback": {
                "blockReason": "SAFETY",
                "safetyRatings": [
                    {
                        "category": "violence",
                        "probability": "low",
                        "blocked": false
                    }
                ]
            },
            "usageMetadata": {
                "promptTokenCount": 5,
                "cachedContentTokenCount": 3,
                "candidatesTokenCount": 10,
                "totalTokenCount": 18
            }
        }
        "#;

        let response: GenerateContentResponse = serde_json::from_str(json_data).unwrap();

        assert_eq!(response.candidates.len(), 1);
        let candidate = &response.candidates[0];
        assert_eq!(candidate.content.as_ref().unwrap().parts.len(), 1);
        assert_eq!(
            candidate.content.as_ref().unwrap().role.as_ref().unwrap(),
            &Role::Model
        );
        assert_eq!(
            candidate.finish_reason.as_ref().unwrap(),
            &FinishReason::Stop
        );
        assert_eq!(candidate.safety_ratings.as_ref().unwrap().len(), 1);
        assert_eq!(candidate.token_count.unwrap(), 10);
        assert_eq!(candidate.index.unwrap(), 0);

        let prompt_feedback = response.prompt_feedback.as_ref().unwrap();
        assert_eq!(
            prompt_feedback.block_reason.as_ref().unwrap(),
            &BlockReason::Safety
        );
        assert_eq!(prompt_feedback.safety_ratings.len(), 1);

        let usage_metadata = response.usage_metadata.as_ref().unwrap();
        assert_eq!(usage_metadata.prompt_token_count.unwrap(), 5);
        assert_eq!(usage_metadata.cached_content_token_count.unwrap(), 3);
        assert_eq!(usage_metadata.candidates_token_count.unwrap(), 10);
        assert_eq!(usage_metadata.total_token_count.unwrap(), 18);
    }
}
