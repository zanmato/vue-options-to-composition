use super::Transformer;
use super::TransformerOrchestrator;
use crate::{TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting Nuxt.js head() method to Composition API useHead
///
/// This transformer handles the conversion from Nuxt.js `head()` method to the Composition API
/// `useHead` composable by:
/// - Adding `import { useHead } from '@unhead/vue';` import
/// - Converting the head() method body to a useHead(() => { ... }) call
/// - Applying i18n and other transformations to the head method body
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::{
///     parse_sfc_sections, parse_script_section, parse_template_section,
///     ScriptParsingState, TemplateParsingState, TransformationContext, TransformerConfig
/// };
/// use vue_options_to_composition::transformers::{Transformer, head::HeadTransformer};
///
/// let sfc = r#"<template><h1>{{ title }}</h1></template>
///     <script>
///     export default {
///       head() {
///         return {
///           title: this.$t('page.title')
///         };
///       }
///     }
///     </script>"#;
///
/// // Parse the SFC
/// let sections = parse_sfc_sections(sfc).expect("Failed to parse SFC");
/// let mut script_state = ScriptParsingState::new();
/// let mut template_state = TemplateParsingState::new();
///
/// if let Some(script_content) = &sections.script_content {
///     parse_script_section(script_content, &mut script_state)
///         .expect("Failed to parse script");
/// }
/// if let Some(template_content) = &sections.template_content {
///     parse_template_section(template_content, &mut template_state)
///         .expect("Failed to parse template");
/// }
///
/// // Create transformation context
/// let context = TransformationContext {
///     script_state,
///     template_state,
///     sfc_sections: sections,
/// };
///
/// let config = TransformerConfig::default();
///
/// // Test the transformer
/// let transformer = HeadTransformer::new();
///
/// // Should detect head method to transform
/// assert!(transformer.should_transform(&context, &config));
///
/// // Transform the component
/// let result = transformer.transform(&context, &config);
///
/// // Should generate useHead call
/// assert!(result.methods.iter().any(|line| line.contains("useHead(() => {")));
/// ```
pub struct HeadTransformer;

impl Default for HeadTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Apply transformations to head method body (i18n, reactive refs, etc.)
  fn transform_head_body(
    &self,
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    // Use the orchestrator's body transformer which applies all available transforms
    let body_transformer = TransformerOrchestrator::get_body_transformer();
    let mut transformed_body = body_transformer(body, context, config);

    // Add trailing commas to object properties for better formatting
    transformed_body = self.add_trailing_commas(&transformed_body);

    transformed_body
  }

  /// Add trailing commas to object properties
  fn add_trailing_commas(&self, body: &str) -> String {
    let result = body.to_string();

    // Simple regex-like replacement for basic cases
    // This handles cases like "property: value" -> "property: value,"
    let lines: Vec<&str> = result.lines().collect();
    let mut new_lines = Vec::new();

    for (i, line) in lines.iter().enumerate() {
      let trimmed = line.trim();
      // If this line looks like a property assignment and the next line is a closing brace
      if trimmed.contains(':') && !trimmed.ends_with(',') && !trimmed.ends_with('{') {
        // Check if the next non-empty line is a closing brace
        let mut found_closing = false;
        for j in (i + 1)..lines.len() {
          let next_trimmed = lines[j].trim();
          if !next_trimmed.is_empty() {
            if next_trimmed.starts_with('}') {
              found_closing = true;
            }
            break;
          }
        }

        if found_closing {
          new_lines.push(format!("{},", line));
        } else {
          new_lines.push(line.to_string());
        }
      } else {
        new_lines.push(line.to_string());
      }
    }

    new_lines.join("\n")
  }
}

impl Transformer for HeadTransformer {
  fn name(&self) -> &'static str {
    "head"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    // Transform if we have a head method
    context.script_state.head_method.is_some()
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    if let Some(head_method) = &context.script_state.head_method {
      // Add useHead import
      result.add_import("@unhead/vue", "useHead");

      // Transform the head method body
      let transformed_body = self.transform_head_body(&head_method.body, context, config);

      // Generate useHead call
      result.methods.push("useHead(() => {".to_string());

      // Add the transformed body with proper indentation
      for line in transformed_body.lines() {
        if !line.trim().is_empty() {
          result.methods.push(format!("  {}", line));
        }
      }

      result.methods.push("});".to_string());
    }

    result
  }
}
