use crate::models::{GitLabBlobResult, Project};
use reqwest::{Client, header};
use std::error::Error;

pub struct GitLabClient {
    client: Client,
    base_url: String,
}

impl GitLabClient {
    pub fn new(
        token: String,
        base_url: Option<String>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut headers = header::HeaderMap::new();
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = Client::builder().default_headers(headers).build()?;

        let mut base_url = base_url.unwrap_or_else(|| "https://gitlab.com/api/v4".to_string());

        if base_url.ends_with('/') {
            base_url.pop();
        }

        if !base_url.ends_with("/api/v4") {
            println!("Note: Appending '/api/v4' to GitLab URL");
            base_url.push_str("/api/v4");
        }

        println!("Using GitLab API at: {}", base_url);

        Ok(Self { client, base_url })
    }

    /// Fetches all projects based on the scope.
    /// If `group_id` is provided, fetches projects for that group (and subgroups).
    /// If `group_id` is None, fetches all projects accessible to the user (membership=true).
    pub async fn get_projects(
        &self,
        group_id: Option<&str>,
    ) -> Result<Vec<Project>, Box<dyn Error + Send + Sync>> {
        let mut projects = Vec::new();
        let mut page = 1;

        let endpoint = if let Some(gid) = group_id {
            format!("{}/groups/{}/projects", self.base_url, gid)
        } else {
            format!("{}/projects", self.base_url)
        };

        loop {
            // println!("Fetching projects page {}...", page); // Debug logging
            let request = self
                .client
                .get(&endpoint)
                .query(&[("per_page", "100"), ("page", &page.to_string())]);

            // Add specific filters based on mode
            let request = if group_id.is_some() {
                request.query(&[("include_subgroups", "true")])
            } else {
                request.query(&[("membership", "true")])
            };

            let response = request.send().await?;

            if !response.status().is_success() {
                return Err(format!("Failed to fetch projects: {}", response.status()).into());
            }

            // Check for pagination header before consuming the body
            let next_page = response
                .headers()
                .get("x-next-page")
                .and_then(|h| h.to_str().ok())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let page_projects: Vec<Project> = match response.json().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(format!(
                        "Failed to parse projects JSON: {}. \nHint: Check if your GITLAB_URL ({}) is correct and accessible. If you see HTML in debug output, it might be a login page or 404.",
                        e,
                        self.base_url
                    )
                    .into());
                }
            };
            if page_projects.is_empty() {
                break;
            }
            projects.extend(page_projects);

            if let Some(next) = next_page {
                page = next.parse().unwrap_or(page + 1);
            } else {
                break;
            }
        }

        Ok(projects)
    }

    /// Searches for a query string within a specific project's blobs.
    pub async fn search_in_project(
        &self,
        project_id: u64,
        query: &str,
    ) -> Result<Vec<GitLabBlobResult>, Box<dyn Error + Send + Sync>> {
        let endpoint = format!("{}/projects/{}/search", self.base_url, project_id);
        let mut retry_count = 0;
        let max_retries = 5;

        loop {
            let response = self
                .client
                .get(&endpoint)
                .query(&[("scope", "blobs"), ("search", query)])
                .send()
                .await?;

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if retry_count >= max_retries {
                    return Err(format!(
                        "Max retries exceeded for project {}: 429 Too Many Requests",
                        project_id
                    )
                    .into());
                }

                let wait_time = if let Some(retry_after) = response.headers().get("retry-after") {
                    retry_after
                        .to_str()
                        .unwrap_or("1")
                        .parse::<u64>()
                        .unwrap_or(1)
                } else {
                    // Exponential backoff: 2^retry_count
                    2_u64.pow(retry_count)
                };

                eprintln!(
                    "\n[429] Rate limited on project {}. Retrying in {}s...",
                    project_id, wait_time
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
                retry_count += 1;
                continue;
            }

            if !response.status().is_success() {
                // If search is disabled or fails for a project, logging it and returning empty might be safer than crashing
                // But let's return error for now to handle it at call site
                return Err(format!(
                    "Search failed for project {}: {}",
                    project_id,
                    response.status()
                )
                .into());
            }

            let results: Vec<GitLabBlobResult> = response.json().await?;
            return Ok(results);
        }
    }
}
