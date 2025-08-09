use super::Transformer;
use crate::{TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting Vue 2 mixins to Vue 3 composables
///
/// This transformer detects mixin imports and usage, then converts them to
/// composable-based patterns using user-provided configuration.
pub struct MixinTransformer;

impl Default for MixinTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl MixinTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains mixin usage based on configuration
  fn has_mixin_usage(&self, context: &TransformationContext, config: &TransformerConfig) -> bool {
    if let Some(mixin_configs) = &config.mixins {
      // Check if any configured mixins are imported
      for import_info in &context.script_state.imports {
        for mixin_name in mixin_configs.keys() {
          if import_info.source.contains(mixin_name) {
            return true;
          }
        }
      }
    }
    false
  }

  /// Extract mixin name from import path
  /// E.g., "@/mixins/price" -> "price"
  fn extract_mixin_name_from_path<'a>(&self, import_path: &'a str) -> Option<&'a str> {
    if let Some(last_part) = import_path.split('/').next_back() {
      return Some(last_part);
    }
    None
  }

  /// Find which mixin functions are used in the code
  fn find_used_mixin_functions(
    &self,
    context: &TransformationContext,
    mixin_functions: &[String],
  ) -> Vec<String> {
    let mut used_functions = Vec::new();

    // Check identifiers for exact function matches
    for identifier in &context.script_state.identifiers {
      if mixin_functions.contains(identifier) {
        used_functions.push(identifier.clone());
      }
    }

    // Check function calls for exact function matches (look for "functionName(" pattern)
    for function_call in &context.script_state.function_calls {
      for mixin_function in mixin_functions {
        if (function_call == mixin_function || function_call.starts_with(&format!("{}(", mixin_function))) && !used_functions.contains(mixin_function) {
          used_functions.push(mixin_function.clone());
        }
      }
    }

    // Check template identifiers for exact function matches
    for identifier in &context.template_state.identifiers {
      if mixin_functions.contains(identifier) && !used_functions.contains(identifier) {
        used_functions.push(identifier.clone());
      }
    }

    // Check template function calls for exact function matches
    for function_call in &context.template_state.function_calls {
      for mixin_function in mixin_functions {
        if (function_call == mixin_function || function_call.starts_with(&format!("{}(", mixin_function))) && !used_functions.contains(mixin_function) {
          used_functions.push(mixin_function.clone());
        }
      }
    }

    used_functions
  }
}

impl Transformer for MixinTransformer {
  fn name(&self) -> &'static str {
    "mixin"
  }

  fn should_transform(&self, context: &TransformationContext, config: &TransformerConfig) -> bool {
    self.has_mixin_usage(context, config)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    if let Some(mixin_configs) = &config.mixins {
      // Find mixin imports and their usage
      for import_info in &context.script_state.imports {
        if let Some(mixin_name) = self.extract_mixin_name_from_path(&import_info.source) {
          if let Some(mixin_config) = mixin_configs.get(mixin_name) {
            // Find which functions from this mixin are actually used
            let used_functions = self.find_used_mixin_functions(context, &mixin_config.imports);

            if !used_functions.is_empty() {
              // Add import for the composable
              result.add_import(
                &format!("@/composables/{}", mixin_config.name),
                &mixin_config.name,
              );

              // Generate destructuring assignment for used functions
              let destructuring = if used_functions.len() == 1 {
                format!(
                  "const {{ {} }} = {}();",
                  used_functions[0], mixin_config.name
                )
              } else {
                let functions_list = used_functions.join(", ");
                format!("const {{ {} }} = {}();", functions_list, mixin_config.name)
              };

              result.setup.push(destructuring);
              
              // Mark these functions as resolved so they don't get FIXME comments
              result.resolved_identifiers.extend(used_functions.clone());
              
              // Also mark them as resolved in skip_data_properties as a fallback
              result.skip_data_properties.extend(used_functions);
            }
          }
        }
      }
    }

    result
  }
}
