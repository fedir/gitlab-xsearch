use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub path_with_namespace: String,
    #[allow(dead_code)]
    pub web_url: String,
    pub http_url_to_repo: String,
    pub path: String, // project slug/folder name
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitLabBlobResult {
    // GitLab search API response structure (abbreviated)
    pub filename: String,
    pub startline: Option<u64>,
    #[allow(dead_code)]
    pub project_id: u64,
    pub data: String, // The snippet
}

// Internal result row for standardized output
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultRow {
    pub group_path: String,
    pub project_name: String,
    pub project_id: u64,
    pub file_name: String,
    pub line_number: u64,
    pub snippet: String,
    pub clone_url: String,
    pub project_folder: String, // default clone folder name
}

impl SearchResultRow {
    pub fn from_api_result(project: &Project, blob: &GitLabBlobResult) -> Self {
        Self {
            group_path: project
                .path_with_namespace
                .split('/')
                .next()
                .unwrap_or("")
                .to_string(),
            project_name: project.name.clone(),
            project_id: project.id,
            file_name: blob.filename.clone(),
            line_number: blob.startline.unwrap_or(0),
            snippet: blob.data.clone(),
            clone_url: project.http_url_to_repo.clone(),
            project_folder: project.path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_row_conversion() {
        let project = Project {
            id: 123,
            name: "Test Project".to_string(),
            path_with_namespace: "my-group/subgroup/test-project".to_string(),
            web_url: "https://gitlab.com/my-group/subgroup/test-project".to_string(),
            http_url_to_repo: "https://gitlab.com/my-group/subgroup/test-project.git".to_string(),
            path: "test-project".to_string(),
        };

        let blob = GitLabBlobResult {
            filename: "src/main.rs".to_string(),
            startline: Some(10),
            project_id: 123,
            data: "fn main() {}".to_string(),
        };

        let row = SearchResultRow::from_api_result(&project, &blob);

        assert_eq!(row.group_path, "my-group");
        assert_eq!(row.project_name, "Test Project");
        assert_eq!(row.project_id, 123);
        assert_eq!(row.file_name, "src/main.rs");
        assert_eq!(row.line_number, 10);
        assert_eq!(row.snippet, "fn main() {}");
        assert_eq!(row.project_folder, "test-project");
    }

    #[test]
    fn test_project_deserialization() {
        let json = r#"{
            "id": 1,
            "name": "Project 1",
            "path_with_namespace": "group/project-1",
            "web_url": "https://gitlab.com/group/project-1",
            "http_url_to_repo": "https://gitlab.com/group/project-1.git",
            "path": "project-1"
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.id, 1);
        assert_eq!(project.path, "project-1");
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Markdown,
    Csv,
    Excel,
}
