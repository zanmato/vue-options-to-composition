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
      // Check for Nuxt-style asset paths
      template_content.contains("~/assets/") || template_content.contains("~assets/")
    } else {
      false
    }
  }

  /// Generate template replacements for asset paths
  fn generate_template_replacements(
    &self,
    _context: &TransformationContext,
  ) -> Vec<TemplateReplacement> {
    // Transform Nuxt-style asset paths to standard Vue paths
    vec![
      TemplateReplacement {
        find: "~/assets/".to_string(),
        replace: "@/assets/".to_string(),
      },
      TemplateReplacement {
        find: "~assets/".to_string(),
        replace: "@/assets/".to_string(),
      },
    ]
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
