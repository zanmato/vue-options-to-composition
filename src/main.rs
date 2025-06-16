use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;
use vue_options_to_composition::{
  rewrite_sfc, AdditionalImport, ImportRewrite, MixinConfig, RewriteOptions,
};
use walkdir::WalkDir;

#[derive(Debug, Deserialize, Serialize)]
struct CliConfig {
  mixins: Option<HashMap<String, CliMixinConfig>>,
  imports_rewrite: Option<HashMap<String, CliImportRewrite>>,
  additional_imports: Option<HashMap<String, CliAdditionalImport>>,
  import_keeplist: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CliMixinConfig {
  name: String,
  imports: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CliImportRewrite {
  name: String,
  component_rewrite: Option<HashMap<String, String>>,
  directives: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CliAdditionalImport {
  import_path: Option<String>,
  rewrite_to: Option<String>,
}

impl From<CliConfig> for RewriteOptions {
  fn from(cli_config: CliConfig) -> Self {
    RewriteOptions {
      mixins: cli_config.mixins.map(|mixins| {
        mixins
          .into_iter()
          .map(|(k, v)| {
            (
              k,
              MixinConfig {
                name: v.name,
                imports: v.imports,
              },
            )
          })
          .collect()
      }),
      imports_rewrite: cli_config.imports_rewrite.map(|imports| {
        imports
          .into_iter()
          .map(|(k, v)| {
            (
              k,
              ImportRewrite {
                name: v.name,
                component_rewrite: v.component_rewrite,
                directives: v.directives,
              },
            )
          })
          .collect()
      }),
      additional_imports: cli_config.additional_imports.map(|imports| {
        imports
          .into_iter()
          .map(|(k, v)| {
            (
              k,
              AdditionalImport {
                import_path: v.import_path,
                rewrite_to: v.rewrite_to,
              },
            )
          })
          .collect()
      }),
      import_keeplist: cli_config.import_keeplist,
    }
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let matches = Command::new("vue-options-to-composition")
    .version("0.1.0")
    .about("Transform Vue 2 SFC to Vue 3 Composition API")
    .arg(
      Arg::new("input")
        .help("Path to Vue SFC file or directory containing .vue files")
        .required(true)
        .index(1),
    )
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .value_name("FILE")
        .help("Configuration TOML file path"),
    )
    .arg(
      Arg::new("output")
        .short('o')
        .long("output")
        .value_name("PATH")
        .help("Output file/directory path (default: overwrites input)"),
    )
    .arg(
      Arg::new("recursive")
        .short('r')
        .long("recursive")
        .action(ArgAction::SetTrue)
        .num_args(0)
        .help("Process directories recursively"),
    )
    .get_matches();

  let input_path = matches.get_one::<String>("input").unwrap();
  let output_path = matches
    .get_one::<String>("output")
    .map(|s| s.as_str())
    .unwrap_or(input_path);
  let config_path = matches.get_one::<String>("config");
  let recursive = matches.get_flag("recursive");

  // Load configuration if provided
  let config = if let Some(config_path) = config_path {
    Some(load_config(config_path).await?)
  } else {
    None
  };

  let success_count = process_path(input_path, output_path, config, recursive).await?;

  if success_count == 0 {
    std::process::exit(1);
  }

  Ok(())
}

async fn load_config(config_path: &str) -> Result<RewriteOptions> {
  let resolved_path = Path::new(config_path)
    .canonicalize()
    .with_context(|| format!("Configuration file not found: {}", config_path))?;

  println!("📝 Loaded configuration from: {}", resolved_path.display());

  let config_content = async_fs::read_to_string(&resolved_path)
    .await
    .with_context(|| format!("Failed to read configuration file: {}", config_path))?;

  let cli_config: CliConfig = toml::from_str(&config_content)
    .with_context(|| format!("Invalid TOML in configuration file: {}", config_path))?;

  Ok(cli_config.into())
}

async fn find_vue_files(dir_path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
  let mut vue_files = Vec::new();

  if recursive {
    for entry in WalkDir::new(dir_path)
      .into_iter()
      .filter_entry(|e| {
        // Skip common directories that shouldn't contain Vue source files
        let name = e.file_name();
        name != "node_modules" && name != ".git" && name != "dist" && name != "build"
      })
      .filter_map(|e| e.ok())
    {
      let path = entry.path();
      if path.is_file() && path.extension().is_some_and(|ext| ext == "vue") {
        vue_files.push(path.to_path_buf());
      }
    }
  } else {
    let mut entries = async_fs::read_dir(dir_path)
      .await
      .with_context(|| format!("Error reading directory: {}", dir_path.display()))?;

    while let Some(entry) = entries.next_entry().await? {
      let path = entry.path();
      if path.is_file() && path.extension().is_some_and(|ext| ext == "vue") {
        vue_files.push(path);
      }
    }
  }

  Ok(vue_files)
}

async fn transform_file(
  input_path: &Path,
  output_path: &Path,
  config: Option<RewriteOptions>,
) -> Result<bool> {
  let resolved_input = input_path
    .canonicalize()
    .with_context(|| format!("Input file not found: {}", input_path.display()))?;

  println!("📄 Transforming: {}", resolved_input.display());

  // Read the input file
  let content = async_fs::read_to_string(&resolved_input)
    .await
    .with_context(|| format!("Failed to read file: {}", resolved_input.display()))?;

  // Transform the content using rewrite_sfc
  let transformed = rewrite_sfc(&content, config)
    .map_err(|e| anyhow::format_err!("{}", e))
    .with_context(|| format!("Failed to transform file: {}", resolved_input.display()))?;

  // Ensure output directory exists
  if let Some(output_dir) = output_path.parent() {
    async_fs::create_dir_all(output_dir)
      .await
      .with_context(|| {
        format!(
          "Failed to create output directory: {}",
          output_dir.display()
        )
      })?;
  }

  // Write the transformed content
  async_fs::write(output_path, transformed)
    .await
    .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

  if resolved_input
    == output_path
      .canonicalize()
      .unwrap_or_else(|_| output_path.to_path_buf())
  {
    println!("   ✅ Overwritten {} successfully", output_path.display());
  } else {
    println!("   ✅ Written to: {}", output_path.display());
  }

  Ok(true)
}

async fn process_path(
  input_path: &str,
  output_path: &str,
  config: Option<RewriteOptions>,
  recursive: bool,
) -> Result<usize> {
  let input_path = Path::new(input_path);
  let output_path = Path::new(output_path);

  let input_metadata = input_path
    .metadata()
    .with_context(|| format!("Path not found: {}", input_path.display()))?;

  if input_metadata.is_file() {
    // Single file processing
    if input_path.extension().is_none_or(|ext| ext != "vue") {
      println!("Warning: Input file does not have a .vue extension");
    }

    let success = transform_file(input_path, output_path, config)
      .await
      .map_err(|e| {
        eprintln!("   ❌ Error: {}", e);
        e
      })
      .unwrap_or(false);

    Ok(if success { 1 } else { 0 })
  } else if input_metadata.is_dir() {
    // Directory processing
    println!("🔍 Searching for .vue files in: {}", input_path.display());

    let vue_files = find_vue_files(input_path, recursive).await?;

    if vue_files.is_empty() {
      println!("No .vue files found in the specified directory.");
      return Ok(0);
    }

    println!("Found {} .vue file(s)", vue_files.len());
    let total_files = vue_files.len();

    // Create tasks for parallel processing
    let mut tasks = Vec::new();

    for vue_file in vue_files {
      // Calculate output path
      let output_file = if input_path == output_path {
        // Overwrite in place
        vue_file.clone()
      } else {
        // Map to output directory structure
        let relative_path = vue_file.strip_prefix(input_path).with_context(|| {
          format!(
            "Failed to calculate relative path for: {}",
            vue_file.display()
          )
        })?;
        output_path.join(relative_path)
      };

      // Spawn a task for each file transformation
      let config_cloned = config.clone();
      let task = tokio::spawn(async move {
        transform_file(&vue_file, &output_file, config_cloned)
          .await
          .map_err(|e| {
            eprintln!("   ❌ Error: {}", e);
            e
          })
          .unwrap_or(false)
      });

      tasks.push(task);
    }

    // Wait for all tasks to complete and count successes
    let mut success_count = 0;
    for task in tasks {
      if let Ok(success) = task.await {
        if success {
          success_count += 1;
        }
      }
    }

    println!(
      "\n📊 Summary: {}/{} files transformed successfully",
      success_count, total_files
    );
    Ok(success_count)
  } else {
    anyhow::bail!("Input path is neither a file nor a directory");
  }
}
