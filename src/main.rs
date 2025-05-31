use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware, web};
use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use tokio::fs;
use tracing::{Level, error, info, warn};

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
struct Config {
    libs_path: PathBuf,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    update_on_start: bool,
    projects: Vec<ProjectConfig>,
}

fn default_port() -> u16 {
    8080
}

#[derive(Debug, Deserialize, Clone)]
struct ProjectConfig {
    path: String,
    repo: Option<String>,
    build_system: BuildSystem,
    #[serde(default)]
    build_command: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum BuildSystem {
    Gradle,
    Cargo,
    Custom,
}

#[derive(Debug, Clone)]
struct Project {
    config: ProjectConfig,
    docs_path: PathBuf,
    url_path: String,
}

#[derive(Debug)]
struct AppState {
    projects: HashMap<String, Project>,
    base_path: PathBuf,
}

fn sanitize_path(path: &str) -> String {
    let mut sanitized = String::with_capacity(path.len());
    let mut last_was_dash = false;

    for c in path.chars() {
        if c.is_ascii_alphanumeric() {
            sanitized.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            sanitized.push('-');
            last_was_dash = true;
        }
    }

    // trim trailing dash if exists
    if sanitized.ends_with('-') {
        sanitized.pop();
    }

    sanitized
}

async fn update_project(path: &Path, repo_url: &str) -> Result<()> {
    let repo = git2::Repository::open(path).or_else(|_| git2::Repository::clone(repo_url, path))?;

    repo.find_remote("origin")?
        .fetch(&["main", "master"], None, None)?;

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let analysis = repo.merge_analysis(&[&commit])?;

    if analysis.0.is_up_to_date() {
        info!("Repository at {} is up-to-date", path.display());
    } else if analysis.0.is_fast_forward() {
        let mut reference = repo.find_reference("refs/heads/main")?;
        reference.set_target(commit.id(), "Fast-Forward")?;
        repo.set_head(reference.name().unwrap())?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        info!("Fast-forwarded repository at {}", path.display());
    } else {
        return Err(anyhow!("Non-fast-forward update required"));
    }

    Ok(())
}

async fn build_docs(project: &ProjectConfig, base_path: &Path) -> Result<()> {
    let project_path = base_path.join(&project.path);

    match project.build_system {
        BuildSystem::Gradle => {
            let gradlew = project_path.join("gradlew");
            if gradlew.exists() {
                tokio::process::Command::new(gradlew)
                    .arg("clean")
                    .arg("javadoc")
                    .current_dir(&project_path)
                    .status()
                    .await?;
            } else {
                tokio::process::Command::new("gradle")
                    .arg("clean")
                    .arg("javadoc")
                    .current_dir(&project_path)
                    .status()
                    .await?;
            }
        }
        BuildSystem::Cargo => {
            tokio::process::Command::new("cargo")
                .arg("doc")
                .current_dir(&project_path)
                .status()
                .await?;
        }
        BuildSystem::Custom => {
            if let Some(cmd) = &project.build_command {
                let mut parts = cmd.split_whitespace();
                if let Some(program) = parts.next() {
                    tokio::process::Command::new(program)
                        .args(parts)
                        .current_dir(&project_path)
                        .status()
                        .await?;
                }
            }
        }
    }

    Ok(())
}

async fn load_config() -> AppResult<Config> {
    let config_str = fs::read_to_string("config.toml")
        .await
        .context("Failed to read config.toml")?;
    let config: Config = toml::from_str(&config_str)?;
    Ok(config)
}

async fn initialize_projects(config: &Config) -> AppResult<HashMap<String, Project>> {
    let mut projects = HashMap::new();

    for project_cfg in &config.projects {
        let url_path = sanitize_path(&project_cfg.path);
        let project_path = config.libs_path.join(&project_cfg.path);

        let docs_path = match project_cfg.build_system {
            BuildSystem::Gradle => project_path.join("build/docs/javadoc"),
            BuildSystem::Cargo => project_path.join("target/doc"),
            BuildSystem::Custom => project_path.join("docs"),
        };

        let project = Project {
            config: project_cfg.clone(),
            docs_path,
            url_path: url_path.clone(),
        };

        projects.insert(url_path, project);
    }

    Ok(projects)
}

#[get("/")]
async fn index(state: web::Data<Arc<AppState>>) -> impl Responder {
    let projects = state
        .projects
        .values()
        .map(|p| {
            format!(
                "<li><a href=\"/{}/\">{}</a></li>",
                p.url_path, p.config.path
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    HttpResponse::Ok().content_type("text/html").body(format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Documentation Server</title>
            <style>
                body {{ font-family: sans-serif; max-width: 800px; margin: 2em auto; }}
                h1 {{ text-align: center; }}
                ul {{ list-style: none; padding: 0; }}
                li {{ margin: 0.5em 0; padding: 0.5em; background: #f5f5f5; border-radius: 4px; }}
                a {{ text-decoration: none; color: #0366d6; font-weight: 500; }}
            </style>
        </head>
        <body>
            <h1>Documentation Server</h1>
            <ul>{}</ul>
        </body>
        </html>
    "#,
        projects
    ))
}

#[actix_web::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let config = load_config().await?;
    let projects = initialize_projects(&config).await?;
    let base_path = config.libs_path.clone();

    if config.update_on_start {
        info!("Updating and building projects...");
        for project in projects.values() {
            let path_str = &project.config.path;
            let repo_url = if let Some(url) = &project.config.repo {
                url
            } else {
                warn!("Skipping {} (no repo URL)", path_str);
                continue;
            };

            info!("Updating {} from {}", path_str, repo_url);
            let project_path = base_path.join(path_str);
            if let Err(e) = update_project(&project_path, repo_url).await {
                error!("Failed to update {}: {}", path_str, e);
            }

            info!("Building docs for {}", path_str);
            if let Err(e) = build_docs(&project.config, &base_path).await {
                error!("Failed to build {}: {}", path_str, e);
            }
        }
    }

    let state = Arc::new(AppState {
        projects,
        base_path,
    });

    info!("Starting server on port {}", config.port);
    HttpServer::new(move || {
        let state = web::Data::new(state.clone());

        // create routes for each project
        let mut app = App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .service(index);

        for project in state.projects.values() {
            let docs_path = project.docs_path.clone();
            let route = project.url_path.clone();
            let resource_path = format!("/{}", route);

            // closure with captured variables for each project
            let route_clone = route.clone();
            app = app.service(web::resource(&resource_path).to(move || {
                let route = route_clone.clone();
                async move {
                    HttpResponse::Found()
                        .append_header(("Location", format!("/{}/", route)))
                        .finish()
                }
            }));

            // closure for the default handler
            let route_clone2 = route.clone();
            app = app.service(
                Files::new(&format!("/{}", route), docs_path)
                    .index_file("index.html")
                    .default_handler(web::to(move || {
                        let route = route_clone2.clone();
                        async move {
                            HttpResponse::Found()
                                .append_header(("Location", format!("/{}/", route)))
                                .finish()
                        }
                    })),
            );
        }

        app
    })
    .bind(("0.0.0.0", config.port))?
    .run()
    .await?;

    Ok(())
}
