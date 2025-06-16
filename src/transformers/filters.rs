use super::Transformer;
use crate::{TransformationContext, TransformationResult, TransformerConfig};
use std::collections::HashSet;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref FILTER_THIS_OPTIONS_PATTERN: Regex = Regex::new(r"this\.\$options\.filters\.(\w+)\s*\(").unwrap();
    static ref FILTER_THIS_OPTIONS_SIMPLE: Regex = Regex::new(r"this\.\$options\.filters\.(\w+)").unwrap();
    static ref FILTER_CALL_PATTERN: Regex = Regex::new(r"(\w+)\(\s*\n\s*([^)]+)\s*\n\s*\)").unwrap();
    static ref FILTER_WHITESPACE_PATTERN: Regex = Regex::new(r"\s+").unwrap();
}

/// Transformer for Vue 2 filters usage
///
/// This transformer handles:
/// - Converting `this.$options.filters.filterName()` to `filterName()` via useFilters composable
/// - Adding `useFilters()` import from '@/composables/useFilters'
/// - Extracting filter names used in the component
pub struct FiltersTransformer;

impl Default for FiltersTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl FiltersTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Extract filter names used in the component
  fn extract_filter_names(&self, context: &TransformationContext) -> HashSet<String> {
    let mut filter_names = HashSet::new();

    // Check method details for filter usage
    for method_detail in &context.script_state.method_details {
      self.extract_filters_from_body(&method_detail.body, &mut filter_names);
    }

    // Check computed properties for filter usage
    for computed_detail in &context.script_state.computed_details {
      if let Some(getter) = &computed_detail.getter {
        self.extract_filters_from_body(getter, &mut filter_names);
      }
      if let Some(setter) = &computed_detail.setter {
        self.extract_filters_from_body(setter, &mut filter_names);
      }
    }

    filter_names
  }

  /// Extract filter names from a body of code
  fn extract_filters_from_body(&self, body: &str, filter_names: &mut HashSet<String>) {
    // Look for patterns like: this.$options.filters.filterName(
    let pattern = &*FILTER_THIS_OPTIONS_PATTERN;

    for captures in pattern.captures_iter(body) {
      if let Some(filter_name) = captures.get(1) {
        filter_names.insert(filter_name.as_str().to_string());
      }
    }
  }

  /// Get body transformation function for converting filter calls
  fn get_filters_body_transform(
  ) -> Box<dyn Fn(&str, &TransformationContext, &TransformerConfig) -> String> {
    Box::new(
      |body: &str, _context: &TransformationContext, _config: &TransformerConfig| {
        let mut transformed_body = body.to_string();

        // Transform filter calls: this.$options.filters.filterName( -> filterName(
        let pattern = &*FILTER_THIS_OPTIONS_SIMPLE;
        transformed_body = pattern.replace_all(&transformed_body, "$1").to_string();

        // Normalize specific filter function calls to be on single lines
        // Look for patterns like: filterName(\n  args\n) and compact them
        let filter_call_pattern = &*FILTER_CALL_PATTERN;
        transformed_body = filter_call_pattern
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let function_name = &caps[1];
            let args = caps[2].replace('\n', " ").trim().to_string();
            // Clean up multiple spaces
            let clean_args = FILTER_WHITESPACE_PATTERN.replace_all(&args, " ");
            format!("{}({})", function_name, clean_args.trim())
          })
          .to_string();

        transformed_body
      },
    )
  }

  /// Check if the component uses filters
  fn has_filter_usage(&self, context: &TransformationContext) -> bool {
    !self.extract_filter_names(context).is_empty()
  }
}

impl Transformer for FiltersTransformer {
  fn name(&self) -> &'static str {
    "filters"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_filter_usage(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Extract filter names used in the component
    let filter_names = self.extract_filter_names(context);

    if !filter_names.is_empty() {
      // Add useFilters import
      result.add_import("@/composables/useFilters", "useFilters");

      // Generate useFilters destructuring
      let mut filter_names_vec: Vec<String> = filter_names.into_iter().collect();
      filter_names_vec.sort(); // Sort for consistent output

      result.setup.push(format!(
        "const {{ {} }} = useFilters();",
        filter_names_vec.join(", ")
      ));
      result.setup.push("".to_string()); // Empty line for readability
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<super::BodyTransformFn>> {
    Some(Self::get_filters_body_transform())
  }
}
