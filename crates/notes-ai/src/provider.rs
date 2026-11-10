use anyhow::{Result, anyhow, bail};
use nyx_ai_client::{
    Client,
    proto::{self, ChatRequest, ErrorKind, Message, Priority, Role, Sampling},
};
pub use nyx_ai_client::proto::{Capability, Model};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, future::Future, time::Duration};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Capabilities {
    pub api_version: u32,
    pub generation: bool,
    pub local: bool,
    pub embedding: bool,
    #[serde(default)]
    pub embedding_model: String,
    #[serde(default)]
    pub embedding_profile: String,
    #[serde(default)]
    pub embedding_endpoint: String,
}
#[derive(Clone, Debug)]
pub struct Embeddings {
    pub profile: String,
    pub vectors: Vec<Vec<f32>>,
}
pub trait Provider: Send + Sync {
    fn capabilities(&self) -> Result<Capabilities>;
    fn generate(&self, system: &str, text: &str) -> Result<String>;
    fn embed(&self, input: &[String]) -> Result<Embeddings>;
}
/// Nyx AI Core, the models this computer shares between apps. An empty model is AI
/// Core's default for the role.
#[derive(Clone, Debug, Default)]
pub struct AiCore {
    pub writing_model: String,
    pub embedding_model: String,
}
/// Load settings that change the vectors a model gives (AI Core's own list).
const VECTOR_KEYS: &[&str] = &["pooling", "image-min-tokens", "image-max-tokens"];
impl AiCore {
    fn block<T>(timeout: Duration, work: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async { tokio::time::timeout(timeout, work).await })
            .map_err(|_| anyhow!("Nyx AI Core took too long to answer"))?
    }
    /// What AI Core has, and its default models by role.
    async fn catalogue(client: &Client) -> Result<(Vec<Model>, BTreeMap<String, String>)> {
        let models = client.list_models().await.map_err(explain)?;
        let defaults = client.service_settings().await.map_err(explain)?.defaults;
        Ok((models, defaults))
    }
    pub fn models(&self) -> Result<(Vec<Model>, BTreeMap<String, String>)> {
        Self::block(Duration::from_secs(5), Self::catalogue(&client()))
    }
}
fn client() -> Client {
    Client::new("nyx-notes")
}
pub fn explain(e: proto::Error) -> anyhow::Error {
    match e.kind {
        ErrorKind::Unavailable => anyhow!("Nyx AI Core isn't running; Notes works normally without it"),
        ErrorKind::IncompatibleVersion => anyhow!("Notes and Nyx AI Core don't match ({}); update both", e.message),
        ErrorKind::Busy => anyhow!("Nyx AI Core is busy; try again in a moment"),
        _ => anyhow!("{}", e.message),
    }
}
/// The model for a role: Notes' choice, else AI Core's default for the first of
/// `roles` that has one. Never another model in its place.
pub fn choose(models: &[Model], defaults: &BTreeMap<String, String>, chosen: &str, roles: &[&str], needs: Capability) -> Result<String> {
    let what = if needs == Capability::Embedding { "embedding" } else { "writing" };
    let id = if chosen.is_empty() {
        roles
            .iter()
            .find_map(|r| defaults.get(*r).filter(|m| !m.is_empty()).cloned())
            .ok_or_else(|| anyhow!("Choose a {what} model in Notes' Local AI preferences"))?
    } else {
        chosen.to_owned()
    };
    let Some(model) = models.iter().find(|m| m.id == id) else {
        bail!("{id} isn't in Nyx AI Core any more; choose another {what} model in Notes' Local AI preferences");
    };
    if !model.capabilities.contains(&needs) {
        bail!("{id} can't be the {what} model");
    }
    Ok(id)
}
/// Which vectors a model gives: its file and the load settings that change them. A new
/// profile resets the semantic index.
fn profile(file: &str, size: u64, revision: &Option<String>, architecture: &Option<String>, settings: &BTreeMap<String, String>) -> String {
    let settings: BTreeMap<_, _> = settings.iter().filter(|(k, _)| VECTOR_KEYS.contains(&k.as_str())).collect();
    format!(
        "nyx-ai:{}",
        json!({"file": file, "size": size, "revision": revision, "architecture": architecture, "settings": settings})
    )
}
fn model_profile(model: &Model, effective: &BTreeMap<String, String>) -> String {
    let file = model.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    profile(&file, model.size, &model.revision, &model.architecture, effective)
}
fn identity_profile(id: &proto::ModelIdentity) -> String {
    profile(&id.file, id.size, &id.revision, &id.architecture, &id.settings)
}
impl Provider for AiCore {
    fn capabilities(&self) -> Result<Capabilities> {
        Self::block(Duration::from_secs(5), async {
            let client = client();
            let (models, defaults) = Self::catalogue(&client).await?;
            let writing = choose(&models, &defaults, &self.writing_model, &["chat", "assistant"], Capability::Chat).ok();
            let embedding = choose(&models, &defaults, &self.embedding_model, &["embedding"], Capability::Embedding).ok();
            let mut embedding_profile = String::new();
            if let Some(id) = &embedding
                && let Some(model) = models.iter().find(|m| &m.id == id)
            {
                let settings = client.model_settings(id).await.map_err(explain)?;
                embedding_profile = model_profile(model, &settings.effective);
            }
            Ok(Capabilities {
                api_version: 1,
                generation: writing.is_some(),
                local: true,
                embedding: embedding.is_some(),
                embedding_model: embedding.unwrap_or_default(),
                embedding_profile,
                embedding_endpoint: String::new(),
            })
        })
    }
    fn generate(&self, system: &str, text: &str) -> Result<String> {
        Self::block(Duration::from_secs(125), async {
            let client = client();
            let (models, defaults) = Self::catalogue(&client).await?;
            let model = choose(&models, &defaults, &self.writing_model, &["chat", "assistant"], Capability::Chat)?;
            let request = ChatRequest {
                model,
                messages: vec![Message::text(Role::System, system), Message::text(Role::User, text)],
                sampling: Sampling { temperature: Some(0.25), max_tokens: Some(2048), ..Default::default() },
                thinking: Some(false),
                priority: Priority::Interactive,
                ..Default::default()
            };
            let reply = client.chat(request).await.map_err(explain)?.collect_chat().await.map_err(explain)?;
            if reply.content.len() > 1024 * 1024 {
                bail!("Generated text exceeds limit");
            }
            Ok(reply.content)
        })
    }
    fn embed(&self, input: &[String]) -> Result<Embeddings> {
        Self::block(Duration::from_secs(95), async {
            let client = client();
            let (models, defaults) = Self::catalogue(&client).await?;
            let model = choose(&models, &defaults, &self.embedding_model, &["embedding"], Capability::Embedding)?;
            let priority = if input.len() == 1 { Priority::Interactive } else { Priority::Background };
            let reply = client.embed(&model, input.to_vec(), priority).await.map_err(explain)?;
            validate_vectors(&reply.vectors, input.len())?;
            Ok(Embeddings { profile: identity_profile(&reply.identity), vectors: reply.vectors })
        })
    }
}
pub fn validate_vectors(vectors: &[Vec<f32>], count: usize) -> Result<usize> {
    let dimensions = vectors.first().map_or(0, Vec::len);
    if dimensions == 0
        || dimensions > 16384
        || vectors.len() != count
        || vectors.iter().any(|v| {
            v.len() != dimensions
                || v.iter().any(|n| !n.is_finite())
                || v.iter().map(|n| (*n as f64).powi(2)).sum::<f64>() <= 0.0
        })
    {
        bail!("Invalid embedding count, dimensions, or values");
    }
    Ok(dimensions)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn model(id: &str, capabilities: Vec<Capability>) -> Model {
        serde_json::from_value(json!({
            "id": id, "name": id, "source": "folder", "path": format!("/models/{id}.gguf"), "size": 42,
            "architecture": "gemma3", "capabilities": capabilities, "state": "installed", "revision": "r1",
        }))
        .unwrap()
    }
    #[test]
    fn models_are_chosen_never_substituted() {
        let models = [model("writer", vec![Capability::Chat]), model("vectors", vec![Capability::Embedding])];
        let mut defaults = BTreeMap::new();
        let pick = |d: &BTreeMap<String, String>, chosen: &str, roles: &[&str], needs| choose(&models, d, chosen, roles, needs);
        assert!(pick(&defaults, "", &["chat", "assistant"], Capability::Chat).unwrap_err().to_string().contains("Choose a writing model"));
        defaults.insert("assistant".into(), "writer".into());
        assert_eq!(pick(&defaults, "", &["chat", "assistant"], Capability::Chat).unwrap(), "writer");
        assert!(pick(&defaults, "gone", &["chat"], Capability::Chat).unwrap_err().to_string().contains("isn't in Nyx AI Core"));
        assert!(pick(&defaults, "writer", &["embedding"], Capability::Embedding).unwrap_err().to_string().contains("can't be the embedding model"));
        assert_eq!(pick(&defaults, "vectors", &["embedding"], Capability::Embedding).unwrap(), "vectors");
    }
    #[test]
    fn the_profile_before_embedding_matches_the_one_after() {
        let m = model("vectors", vec![Capability::Embedding]);
        let effective: BTreeMap<String, String> = [("pooling", "mean"), ("ctx-size", "8192")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let identity: proto::ModelIdentity = serde_json::from_value(json!({
            "id": "vectors", "file": "vectors.gguf", "size": 42, "revision": "r1", "architecture": "gemma3",
            "settings": {"pooling": "mean"},
        }))
        .unwrap();
        assert_eq!(model_profile(&m, &effective), identity_profile(&identity));
        let other: BTreeMap<String, String> = [("pooling".to_string(), "cls".to_string())].into();
        assert_ne!(model_profile(&m, &other), identity_profile(&identity));
    }
    /// Lists models and settings only; nothing loads.
    #[test]
    #[ignore = "needs a running Nyx AI Core"]
    fn running_ai_core() {
        let caps = AiCore::default().capabilities().unwrap();
        assert!(caps.local);
        assert_eq!(caps.embedding, !caps.embedding_profile.is_empty());
        let (models, _) = AiCore::default().models().unwrap();
        assert!(!models.is_empty());
    }
    #[test]
    fn vectors_are_checked() {
        assert!(validate_vectors(&[vec![f32::NAN]], 1).is_err());
        assert!(validate_vectors(&[vec![0.0]], 1).is_err());
        assert_eq!(validate_vectors(&[vec![1.0, 0.0]], 1).unwrap(), 2);
    }
}
