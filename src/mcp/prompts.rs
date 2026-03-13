use rmcp::model::{
    AnnotateAble, GetPromptResult, JsonObject, ListPromptsResult, Prompt, PromptArgument,
    PromptMessage, PromptMessageRole, RawResource,
};

use super::TarnMcpServer;

impl TarnMcpServer {
    pub fn list_prompts_static(&self) -> ListPromptsResult {
        ListPromptsResult {
            prompts: vec![
                Prompt::new(
                    "tarn_explore_topic",
                    Some("Guided deep-dive into a topic across the vault. Searches for related notes, reads the most relevant ones, and follows links."),
                    Some(vec![
                        PromptArgument::new("topic")
                            .with_description("Topic to explore")
                            .with_required(true),
                        PromptArgument::new("folder")
                            .with_description("Restrict exploration to a folder")
                            .with_required(false),
                    ]),
                )
                .with_title("Explore Topic"),
                Prompt::new(
                    "tarn_summarize_project",
                    Some("Generate a project status summary by reading all notes in a project folder and analyzing their state."),
                    Some(vec![
                        PromptArgument::new("folder")
                            .with_description("Project folder path")
                            .with_required(true),
                    ]),
                )
                .with_title("Summarize Project"),
            ],
            next_cursor: None,
            meta: None,
        }
    }

    pub fn get_prompt_by_name(
        &self,
        name: &str,
        args: &JsonObject,
    ) -> Result<GetPromptResult, rmcp::ErrorData> {
        match name {
            "tarn_explore_topic" => {
                let topic = args.get("topic").and_then(|v| v.as_str()).ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing required argument: topic", None)
                })?;
                let folder_context = args
                    .get("folder")
                    .and_then(|v| v.as_str())
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
                            "Explore my vault for everything related to \"{topic}\"{folder_context}. \
                            Use tarn_search_notes to find relevant notes, then tarn_read_note with \
                            include_links=true on the top results. Synthesize a summary of what I \
                            know about this topic, what's well-documented, and where there are gaps."
                        ),
                    ),
                ];

                Ok(GetPromptResult::new(messages)
                    .with_description(format!("Explore topic: {topic}")))
            }
            "tarn_summarize_project" => {
                let folder = args.get("folder").and_then(|v| v.as_str()).ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing required argument: folder", None)
                })?;

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
                            "Summarize the project in \"{folder}\". Use tarn_list_notes to find all notes, \
                            tarn_read_note(summary=true) for each, and analyze how they connect. \
                            Produce a status report: key documents, open topics, tag distribution, \
                            and an overview of the project structure."
                        ),
                    ),
                ];

                Ok(GetPromptResult::new(messages)
                    .with_description(format!("Summarize project: {folder}")))
            }
            _ => Err(rmcp::ErrorData::invalid_params(
                format!("unknown prompt: {name}"),
                None,
            )),
        }
    }
}
