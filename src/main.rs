mod client;
mod models;

use clap::{Parser, Subcommand};
use client::GitLabClient;
use comfy_table::{Table, presets::UTF8_FULL};
use futures::{StreamExt, stream};
use models::{OutputFormat, SearchResultRow};
use std::error::Error;
use std::io::stdout;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "gitlab-xsearch")]
#[command(about = "Transversal search across GitLab projects without cloning", long_about = None)]
struct Cli {
    /// GitLab Personal Access Token. Can also be set via GITLAB_TOKEN env var.
    #[arg(long, env = "GITLAB_TOKEN")]
    token: String,

    /// GitLab Base URL. Defaults to https://gitlab.com/api/v4
    #[arg(long, env = "GITLAB_URL")]
    url: Option<String>,

    /// Search query string
    #[arg(long, short = 'q')]
    query: String,

    /// Output format (table, markdown, csv)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Output file path (optional, defaults to stdout)
    #[arg(long, short = 'o')]
    output: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search in all accessible projects
    Global {
        /// Maximum number of projects to search (optional)
        #[arg(long)]
        max: Option<usize>,
    },
    /// Search in a specific group (and subgroups)
    Group {
        /// Group ID or encoded path
        #[arg(value_name = "GROUP_ID")]
        id: String,
        /// Maximum number of projects to search (optional)
        #[arg(long)]
        max: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    // 1. Initialize Client
    let client = Arc::new(GitLabClient::new(cli.token, cli.url)?);

    // 2. Fetch Projects Strategy
    println!("Fetching projects...");
    let projects = match cli.command {
        Commands::Global { max } => {
            let mut p = client.get_projects(None).await?;
            if let Some(m) = max {
                println!("Note: Limited to first {} projects", m);
                p.truncate(m);
            }
            p
        }
        Commands::Group { id, max } => {
            let mut p = client.get_projects(Some(&id)).await?;
            if let Some(m) = max {
                println!("Note: Limited to first {} projects", m);
                p.truncate(m);
            }
            p
        }
    };

    println!(
        "Found {} projects. Starting search for '{}'...",
        projects.len(),
        cli.query
    );

    // 3. Search Concurrently (Batching)
    // We use a stream to limit concurrency so we don't hammer the API too hard or run into file descriptor limits
    const CONCURRENT_REQUESTS: usize = 5;

    let query = Arc::new(cli.query);
    // Initialize progress bar
    let pb = indicatif::ProgressBar::new(projects.len() as u64);
    pb.set_style(indicatif::ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
        .progress_chars("#>-"));

    let results_stream = stream::iter(projects)
        .map(|p| {
            let client = Arc::clone(&client);
            let query = query.clone();
            tokio::spawn(async move {
                match client.search_in_project(p.id, &query).await {
                    Ok(results) => {
                        if !results.is_empty() {
                            Some(
                                results
                                    .into_iter()
                                    .map(move |r| SearchResultRow::from_api_result(&p, &r))
                                    .collect::<Vec<_>>(),
                            )
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!("Error searching project {}: {}", p.name, e);
                        None
                    }
                }
            })
        })
        .buffer_unordered(CONCURRENT_REQUESTS);

    // Collect all results
    let rows: Vec<SearchResultRow> = results_stream
        .inspect(|_| pb.inc(1))
        .filter_map(|res| async {
            match res {
                Ok(opt) => opt,
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    None
                }
            }
        })
        .map(stream::iter)
        .flatten()
        .collect()
        .await;

    pb.finish_with_message("Search done");

    println!("Found {} matches.", rows.len());

    // 4. Output
    match cli.format {
        OutputFormat::Csv => {
            let mut wtr = if let Some(path) = cli.output {
                csv::Writer::from_writer(
                    Box::new(std::fs::File::create(path)?) as Box<dyn std::io::Write>
                )
            } else {
                csv::Writer::from_writer(Box::new(stdout()) as Box<dyn std::io::Write>)
            };

            for row in &rows {
                wtr.serialize(row)?;
            }
            wtr.flush()?;
        }
        OutputFormat::Excel => {
            let path = cli
                .output
                .as_deref()
                .ok_or("Output file path (-o) is required for Excel format")?;
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let worksheet = workbook.add_worksheet();

            // Set headers
            let headers = vec![
                "Group",
                "Project",
                "ID",
                "File",
                "Line",
                "Snippet",
                "Clone URL",
                "Folder",
            ];
            for (col, header) in headers.iter().enumerate() {
                worksheet.write_string(0, col as u16, *header)?;
            }

            // Set data
            for (row_idx, row) in rows.iter().enumerate() {
                let r = (row_idx + 1) as u32;
                worksheet.write_string(r, 0, &row.group_path)?;
                worksheet.write_string(r, 1, &row.project_name)?;
                worksheet.write_number(r, 2, row.project_id as f64)?;
                worksheet.write_string(r, 3, &row.file_name)?;
                worksheet.write_number(r, 4, row.line_number as f64)?;
                worksheet.write_string(r, 5, &row.snippet)?;
                worksheet.write_string(r, 6, &row.clone_url)?;
                worksheet.write_string(r, 7, &row.project_folder)?;
            }

            workbook.save(path)?;
            println!("Excel file saved to {}", path);
        }
        OutputFormat::Table | OutputFormat::Markdown => {
            let mut table = Table::new();
            table
                .load_preset(if matches!(cli.format, OutputFormat::Markdown) {
                    comfy_table::presets::ASCII_MARKDOWN
                } else {
                    UTF8_FULL
                })
                .set_header(vec![
                    "Group",
                    "Project",
                    "ID",
                    "File",
                    "Line",
                    "Snippet",
                    "Clone URL",
                    "Folder",
                ]);

            for row in &rows {
                let snippet = if matches!(cli.format, OutputFormat::Markdown) {
                    row.snippet.replace('\n', "<br>")
                } else {
                    row.snippet.clone()
                };

                table.add_row(vec![
                    &row.group_path,
                    &row.project_name,
                    &row.project_id.to_string(),
                    &row.file_name,
                    &row.line_number.to_string(),
                    &snippet,
                    &row.clone_url,
                    &row.project_folder,
                ]);
            }

            if let Some(path) = cli.output {
                std::fs::write(path, table.to_string())?;
            } else {
                println!("{}", table);
            }
        }
    }

    Ok(())
}
