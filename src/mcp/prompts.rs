use rmcp::{
    handler::server::wrapper::Parameters,
    model::{AnnotateAble, GetPromptResult, PromptMessage, PromptMessageRole, RawResource},
    prompt, prompt_router,
};
use schemars::JsonSchema;

use super::TarnMcpServer;
use crate::index::Index;
use crate::observer::Observer;
use crate::storage::Storage;

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct ExploreTopicArgs {
    #[schemars(description = "Topic to explore")]
    pub topic: String,
    #[schemars(description = "Restrict exploration to a folder")]
    pub folder: Option<String>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct SummarizeProjectArgs {
    #[schemars(description = "Project folder path")]
    pub folder: String,
}

#[prompt_router(vis = "pub(crate)")]
impl<S, I, O> TarnMcpServer<S, I, O>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
{
    #[prompt(
        name = "tarn_explore_topic",
        description = "Guided deep-dive into a topic across the vault. Searches for related notes, \
        reads the most relevant ones, and follows links."
    )]
    fn explore_topic(&self, Parameters(args): Parameters<ExploreTopicArgs>) -> GetPromptResult {
        let folder_context = args
            .folder
            .as_ref()
            .map(|f| format!(" within the folder \"{f}\""))
            .unwrap_or_default();

        let messages = vec![
            PromptMessage::new_resource_link(
                PromptMessageRole::User,
                RawResource::new("tarn://vault/info", "Vault Info").no_annotation(),
            ),
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Explore my vault for everything related to \"{}\"{folder_context}. \
                    Use tarn_search_notes to find relevant notes, then read the top results \
                    via the tarn://note/{{path}} resource. Synthesize a summary of what I \
                    know about this topic, what's well-documented, and where there are gaps.",
                    args.topic
                ),
            ),
        ];

        GetPromptResult::new(messages).with_description(format!("Explore topic: {}", args.topic))
    }

    #[prompt(
        name = "tarn_summarize_project",
        description = "Generate a project status summary by reading all notes in a project folder and analyzing their state."
    )]
    fn summarize_project(
        &self,
        Parameters(args): Parameters<SummarizeProjectArgs>,
    ) -> GetPromptResult {
        let folder = &args.folder;

        let messages = vec![
            PromptMessage::new_resource_link(
                PromptMessageRole::User,
                RawResource::new(format!("tarn://vault/info/{folder}"), "Vault Info")
                    .no_annotation(),
            ),
            PromptMessage::new_resource_link(
                PromptMessageRole::User,
                RawResource::new(format!("tarn://vault/tags/{folder}"), "Vault Tags")
                    .no_annotation(),
            ),
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Summarize the project in \"{folder}\". Use tarn_search_notes(query=\"\", folder=\"{folder}\") \
                    to find all notes, then read each via the tarn://note/{{path}} resource. \
                    Produce a status report: key documents, open topics, tag distribution, \
                    and an overview of the project structure."
                ),
            ),
        ];

        GetPromptResult::new(messages).with_description(format!("Summarize project: {folder}"))
    }
}
