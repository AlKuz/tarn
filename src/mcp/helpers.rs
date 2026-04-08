use crate::common::VaultPath;

/// Parse an optional folder string into a validated `VaultPath`.
pub fn parse_folder(folder: Option<&str>) -> Result<Option<VaultPath>, rmcp::ErrorData> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized)
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))
        })
        .transpose()
}
