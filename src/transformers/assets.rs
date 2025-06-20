use super::Transformer;
use crate::{TemplateReplacement, TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting asset paths and other template transformations
pub struct AssetsTransformer;

impl Default for AssetsTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if there are asset paths that need transformation
  fn has_asset_transformations(&self, context: &TransformationContext) -> bool {
    if let Some(template_content) = &context.sfc_sections.template_content {
      // Check for Nuxt-style asset paths or require() calls
      template_content.contains("~/assets/") 
        || template_content.contains("~assets/")
        || template_content.contains("require(")
    } else {
      false
    }
  }

  /// Generate template replacements for asset paths
  fn generate_template_replacements(
    &self,
    context: &TransformationContext,
  ) -> Vec<TemplateReplacement> {
    let mut replacements = vec![
      // Transform Nuxt-style asset paths to standard Vue paths
      TemplateReplacement {
        find: "~/assets/".to_string(),
        replace: "@/assets/".to_string(),
      },
      TemplateReplacement {
        find: "~assets/".to_string(),
        replace: "@/assets/".to_string(),
      },
    ];

    // Add require() removal replacements if needed
    if let Some(template_content) = &context.sfc_sections.template_content {
      if template_content.contains("require(") {
        // Use regex to handle require() patterns more flexibly
        use regex::Regex;
        
        // Find all require patterns and create specific replacements
        let require_regex = Regex::new(r#":src="require\(([^)]+)\)""#).unwrap();
        for captures in require_regex.captures_iter(template_content) {
          if let Some(path_match) = captures.get(1) {
            let path = path_match.as_str();
            let full_match = captures.get(0).unwrap().as_str();
            
            // Replace :src="require('path')" with src="path" 
            // Extract the path without quotes and add consistent double quotes
            let clean_path = path.trim_matches('\'').trim_matches('"');
            replacements.push(TemplateReplacement {
              find: full_match.to_string(),
              replace: format!("src=\"{}\"", clean_path),
            });
          }
        }
      }
    }

    replacements
  }
}

impl Transformer for AssetsTransformer {
  fn name(&self) -> &'static str {
    "assets"
  }

  fn should_transform(&self, context: &TransformationContext, config: &TransformerConfig) -> bool {
    config.enable_asset_transforms && self.has_asset_transformations(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Generate template replacements
    result
      .template_replacements
      .extend(self.generate_template_replacements(context));

    result
  }
}
