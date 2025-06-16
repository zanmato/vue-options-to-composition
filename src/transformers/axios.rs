use super::{BodyTransformFn, Transformer};
use crate::{TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting axios usage from Options API to Composition API
///
/// This transformer handles the conversion of axios calls from `this.$axios` to a
/// composable-based approach using `useHttp()`.
pub struct AxiosTransformer;

impl Default for AxiosTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl AxiosTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains axios usage
  fn has_axios_usage(&self, context: &TransformationContext) -> bool {
    // Check if there are any $axios calls in the script
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$axios"))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id.contains("$axios"))
  }
}

impl Transformer for AxiosTransformer {
  fn name(&self) -> &'static str {
    "axios"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_axios_usage(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    if !self.has_axios_usage(context) {
      return result;
    }

    // Add useHttp import
    result.add_import("@/composables/useHttp", "useHttp");

    // Add http composable setup
    result.setup.push("const http = useHttp();".to_string());

    result
  }

  fn get_body_transform(&self) -> Option<Box<BodyTransformFn>> {
    Some(Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let axios_transformer = AxiosTransformer::new();
        if !axios_transformer.has_axios_usage(context) {
          return body.to_string();
        }

        // Transform this.$axios calls to http calls
        // Handle cases where this.$axios is followed by newlines and method calls
        let mut result = body.to_string();

        // Then handle any remaining this.$axios occurrences
        result = result.replace("this.$axios", "http");

        result.replace("$axios", "http")
      },
    ))
  }
}
