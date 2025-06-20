use super::{BodyTransformFn, Transformer};
use crate::{TemplateReplacement, TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting Vue Router usage from Options API to Composition API
///
/// This transformer handles the conversion of:
/// - `this.$route` -> `route` (from useRoute())
/// - `this.$router` -> `router` (from useRouter())
pub struct RouterTransformer;

impl Default for RouterTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains $route usage
  fn has_route_usage(&self, context: &TransformationContext) -> bool {
    self.has_route_in_identifiers(context) || self.has_route_in_methods(context) || self.has_route_in_template(context)
  }

  /// Check if context contains $router usage
  fn has_router_usage(&self, context: &TransformationContext) -> bool {
    self.has_router_in_identifiers(context) || self.has_router_in_methods(context) || self.has_router_in_template(context)
  }

  /// Check for $route in identifiers and function calls
  fn has_route_in_identifiers(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .identifiers
      .iter()
      .any(|id| id.contains("$route"))
      || context
        .script_state
        .function_calls
        .iter()
        .any(|call| call.contains("$route"))
  }

  /// Check for $router in identifiers and function calls
  fn has_router_in_identifiers(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .identifiers
      .iter()
      .any(|id| id.contains("$router"))
      || context
        .script_state
        .function_calls
        .iter()
        .any(|call| call.contains("$router"))
  }

  /// Check for $route usage in method bodies
  fn has_route_in_methods(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("$route"))
  }

  /// Check for $router usage in method bodies
  fn has_router_in_methods(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("$router"))
  }

  /// Check for $route usage in template
  fn has_route_in_template(&self, context: &TransformationContext) -> bool {
    if let Some(template_content) = &context.sfc_sections.template_content {
      template_content.contains("$route")
    } else {
      false
    }
  }

  /// Check for $router usage in template
  fn has_router_in_template(&self, context: &TransformationContext) -> bool {
    if let Some(template_content) = &context.sfc_sections.template_content {
      template_content.contains("$router")
    } else {
      false
    }
  }
}

impl Transformer for RouterTransformer {
  fn name(&self) -> &'static str {
    "router"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_route_usage(context) || self.has_router_usage(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    // Add vue-router imports
    let mut imports = Vec::new();
    if self.has_route_usage(context) {
      imports.push("useRoute");
    }
    if self.has_router_usage(context) {
      imports.push("useRouter");
    }

    if !imports.is_empty() {
      result.add_imports("vue-router", &imports);
    }

    // Add composable setup code
    if self.has_route_usage(context) {
      result.add_setup("const route = useRoute();".to_string());
    }
    if self.has_router_usage(context) {
      result.add_setup("const router = useRouter();".to_string());
    }

    // Add blank line after setup if we added anything
    if self.has_route_usage(context) || self.has_router_usage(context) {
      result.add_setup("".to_string());
    }

    // Add template replacements for $route and $router
    if self.has_route_in_template(context) {
      result.template_replacements.push(TemplateReplacement {
        find: "$route".to_string(),
        replace: "route".to_string(),
      });
    }

    if self.has_router_in_template(context) {
      result.template_replacements.push(TemplateReplacement {
        find: "$router".to_string(),
        replace: "router".to_string(),
      });
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<BodyTransformFn>> {
    Some(Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let router_transformer = RouterTransformer::new();
        let mut transformed_body = body.to_string();

        // Transform $route and $router usage
        if router_transformer.has_route_usage(context) {
          transformed_body = transformed_body.replace("this.$route", "route");
          transformed_body = transformed_body.replace("$route", "route");
        }

        if router_transformer.has_router_usage(context) {
          transformed_body = transformed_body.replace("this.$router", "router");
          transformed_body = transformed_body.replace("$router", "router");
        }

        transformed_body
      },
    ))
  }
}
