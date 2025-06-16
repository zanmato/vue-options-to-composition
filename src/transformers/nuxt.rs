use super::{BodyTransformFn, Transformer};
use crate::{TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting Nuxt.js specific features from Options API to Composition API
///
/// This transformer handles the conversion of Nuxt.js specific methods like `fetch()` and
/// converts `this.$fetch()` calls to plain `fetch()` calls.
pub struct NuxtTransformer;

impl Default for NuxtTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl NuxtTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains Nuxt fetch method
  fn has_fetch_method(&self, context: &TransformationContext) -> bool {
    context.script_state.fetch_method.is_some()
  }

  /// Check if context contains Nuxt asyncData method
  fn has_async_data_method(&self, context: &TransformationContext) -> bool {
    context.script_state.async_data_method.is_some()
  }

  /// Check if context contains $fetch calls
  fn has_fetch_calls(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$fetch"))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id.contains("$fetch"))
  }

  /// Check if context contains nuxtI18n configuration
  fn has_nuxt_i18n(&self, context: &TransformationContext) -> bool {
    context.script_state.nuxt_i18n.is_some()
  }

  /// Check if context contains nuxt-link usage in templates
  fn has_nuxt_link_usage(&self, context: &TransformationContext) -> bool {
    // Check template content for nuxt-link tags
    context
      .sfc_sections
      .template_content
      .as_ref()
      .is_some_and(|template| {
        template.contains("nuxt-link") || template.contains("NuxtLink")
      })
  }

  /// Check if context contains $config usage
  fn has_config_usage(&self, context: &TransformationContext) -> bool {
    // Check function calls and identifiers for $config usage
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$config"))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id.contains("$config"))
      // Also check method bodies for $config usage
      || context
        .script_state
        .method_details
        .iter()
        .any(|method| method.body.contains("$config"))
      // Check template for $config usage
      || context
        .template_state
        .function_calls
        .iter()
        .any(|call| call.contains("$config"))
      || context
        .template_state
        .identifiers
        .iter()
        .any(|id| id.contains("$config"))
  }

  /// Generate i18n script block from nuxtI18n configuration
  fn generate_i18n_script(&self, context: &TransformationContext) -> Option<String> {
    if let Some(nuxt_i18n_content) = &context.script_state.nuxt_i18n {
      // Parse the nuxtI18n object to extract paths
      if let Some(paths_content) = self.extract_paths_from_nuxt_i18n(nuxt_i18n_content) {
        // Format the paths content properly
        let formatted_paths = self.format_paths_object(&paths_content);
        let script_block = format!(
          "<script>\nexport const i18n = {};\n</script>",
          formatted_paths
        );
        return Some(script_block);
      }
    }
    None
  }

  /// Extract paths object from nuxtI18n configuration
  fn extract_paths_from_nuxt_i18n(&self, nuxt_i18n_content: &str) -> Option<String> {
    // Simple parsing to extract the paths object
    // Looking for pattern: { paths: { ... }, ... }
    if let Some(paths_start) = nuxt_i18n_content.find("paths:") {
      let paths_section = &nuxt_i18n_content[paths_start + 6..];

      // Find the opening brace after "paths:"
      if let Some(brace_start) = paths_section.find('{') {
        let mut brace_count = 1;
        let mut end_pos = brace_start + 1;
        let chars: Vec<char> = paths_section.chars().collect();

        // Find matching closing brace
        while end_pos < chars.len() && brace_count > 0 {
          match chars[end_pos] {
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            _ => {}
          }
          end_pos += 1;
        }

        if brace_count == 0 {
          let paths_object = &paths_section[brace_start..end_pos];
          return Some(paths_object.to_string());
        }
      }
    }
    None
  }

  /// Format the paths object to match expected output
  fn format_paths_object(&self, paths_content: &str) -> String {
    // Parse the object and reformat it with proper indentation
    let mut result = String::from("{\n");

    // Extract key-value pairs from the paths object
    let content = paths_content
      .trim()
      .trim_start_matches('{')
      .trim_end_matches('}');

    // Split by lines and process each key-value pair
    for line in content.lines() {
      let line = line.trim();
      if !line.is_empty() && line.contains(':') {
        // Clean up the line and ensure proper formatting
        let cleaned_line = line.trim_end_matches(',');
        result.push_str("  ");
        result.push_str(cleaned_line);
        result.push_str(",\n");
      }
    }

    result.push('}');
    result
  }

  /// Generate the fetch method in Composition API style
  fn generate_fetch_method(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<String> {
    let mut setup_code = Vec::new();

    if let Some(fetch_method) = &context.script_state.fetch_method {
      // Transform the method body using the orchestrator's body transformer
      let body_transformer = super::TransformerOrchestrator::get_body_transformer();
      let transformed_body = body_transformer(&fetch_method.body, context, config);

      // Generate the fetch function
      if fetch_method.is_async {
        setup_code.push("const fetch = async () => {".to_string());
      } else {
        setup_code.push("const fetch = () => {".to_string());
      }

      // Add the transformed body (with proper indentation)
      for line in transformed_body.lines() {
        if !line.trim().is_empty() {
          setup_code.push(format!("  {}", line));
        }
      }

      setup_code.push("};".to_string());

      // Add empty line for separation
      setup_code.push("".to_string());

      // Call fetch() immediately at the end
      setup_code.push("fetch();".to_string());
    }

    setup_code
  }

  /// Generate the asyncData method in Composition API style
  fn generate_async_data_method(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> Vec<String> {
    let mut setup_code = Vec::new();

    if let Some(async_data_method) = &context.script_state.async_data_method {
      // Extract the parameters from the asyncData method signature
      let params = self.extract_async_data_params(async_data_method);

      // Extract the method body (everything after the signature)
      let body = self.extract_async_data_body(async_data_method);

      // Note: returned properties are now handled via generate_async_data_refs

      // Generate the useAsyncData call with proper signature
      setup_code.push(format!(
        "const data = await useAsyncData(async ({}) => {{",
        params
      ));

      // Add the transformed body (with proper indentation)
      for line in body.lines() {
        if !line.trim().is_empty() {
          setup_code.push(format!("  {}", line));
        }
      }

      setup_code.push("});".to_string());
      setup_code.push("".to_string()); // Empty line for separation

      // Note: ref assignments are now handled via data_refs map with priority
    }

    setup_code
  }

  /// Generate data refs with priority for asyncData properties
  fn generate_async_data_refs(
    &self,
    context: &TransformationContext,
  ) -> std::collections::HashMap<String, (String, u8)> {
    use std::collections::HashMap;
    let mut data_refs = HashMap::new();

    if let Some(async_data_method) = &context.script_state.async_data_method {
      let body = self.extract_async_data_body(async_data_method);
      let returned_properties = self.extract_returned_properties(&body);

      // Generate ref assignments for returned properties with higher priority (10)
      for prop in returned_properties {
        if context
          .script_state
          .data_properties
          .iter()
          .any(|dp| dp.name == prop)
        {
          let ref_declaration = format!("const {} = ref(data.{});", prop, prop);
          data_refs.insert(prop, (ref_declaration, 10));
        }
      }
    }

    data_refs
  }

  /// Extract parameters from asyncData method signature
  fn extract_async_data_params(&self, method_content: &str) -> String {
    // Look for pattern: asyncData({ param1, param2, ... }) or asyncData(ctx)
    if let Some(params_start) = method_content.find('(') {
      if let Some(params_end) = method_content.find(')') {
        let params_section = &method_content[params_start + 1..params_end];
        return params_section.trim().to_string();
      }
    }
    // Default fallback
    "$axios, app, redirect, params".to_string()
  }

  /// Extract the body content from asyncData method (after the signature)
  fn extract_async_data_body(&self, method_content: &str) -> String {
    // Find the closing parenthesis of the parameters and then the opening brace
    if let Some(params_end) = method_content.find(')') {
      if let Some(brace_start) = method_content[params_end..].find('{') {
        let absolute_brace_start = params_end + brace_start;
        let body_content = &method_content[absolute_brace_start + 1..];
        // Remove the trailing brace if it exists
        let body_content = body_content.trim_end_matches('}').trim();
        return body_content.to_string();
      }
    }

    // Fallback to original logic
    if let Some(brace_start) = method_content.find('{') {
      let body_content = &method_content[brace_start + 1..];
      let body_content = body_content.trim_end_matches('}').trim();
      return body_content.to_string();
    }
    method_content.to_string()
  }

  /// Extract property names from the return statement
  fn extract_returned_properties(&self, body: &str) -> Vec<String> {
    let mut properties = Vec::new();

    // Look for return statement pattern: return { prop1: value1, prop2, ... }
    if let Some(return_pos) = body.rfind("return") {
      let return_section = &body[return_pos..];

      if let Some(brace_start) = return_section.find('{') {
        if let Some(brace_end) = return_section.find('}') {
          let object_content = &return_section[brace_start + 1..brace_end];

          // Parse object properties
          for line in object_content.lines() {
            let line = line.trim().trim_end_matches(',');
            if !line.is_empty() {
              if line.contains(':') {
                // Property with value: prop: value
                if let Some(colon_pos) = line.find(':') {
                  let prop_name = line[..colon_pos].trim();
                  properties.push(prop_name.to_string());
                }
              } else {
                // Shorthand property: prop (equivalent to prop: prop)
                properties.push(line.to_string());
              }
            }
          }
        }
      }
    }

    properties
  }

  /// Check if context contains $nuxt event bus calls
  fn has_nuxt_event_bus(&self, context: &TransformationContext) -> bool {
    // Check function calls for $nuxt.$on, $nuxt.$off, $nuxt.$emit
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| {
        call.contains("$nuxt.$on") || call.contains("$nuxt.$off") || call.contains("$nuxt.$emit")
      })
      // Also check method bodies for $nuxt usage
      || context
        .script_state
        .method_details
        .iter()
        .any(|method| {
          method.body.contains("this.$nuxt.$on")
            || method.body.contains("this.$nuxt.$off")
            || method.body.contains("this.$nuxt.$emit")
        })
  }

  /// Check if context contains $nuxt redirect calls
  fn has_nuxt_redirect(&self, context: &TransformationContext) -> bool {
    // Check function calls for $nuxt.context.redirect
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$nuxt.context.redirect"))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id.contains("$nuxt.context.redirect"))
      // Also check method bodies for $nuxt.context.redirect usage
      || context
        .script_state
        .method_details
        .iter()
        .any(|method| method.body.contains("this.$nuxt.context.redirect"))
  }

  /// Check if context contains $nuxt refresh calls
  fn has_nuxt_refresh(&self, context: &TransformationContext) -> bool {
    // Check function calls for $nuxt.refresh
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$nuxt.refresh"))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id.contains("$nuxt.refresh"))
      // Also check method bodies for $nuxt.refresh usage
      || context
        .script_state
        .method_details
        .iter()
        .any(|method| method.body.contains("this.$nuxt.refresh"))
  }
}

impl Transformer for NuxtTransformer {
  fn name(&self) -> &'static str {
    "nuxt"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_fetch_method(context)
      || self.has_fetch_calls(context)
      || self.has_nuxt_i18n(context)
      || self.has_async_data_method(context)
      || self.has_nuxt_event_bus(context)
      || self.has_config_usage(context)
      || self.has_nuxt_link_usage(context)
      || self.has_nuxt_redirect(context)
      || self.has_nuxt_refresh(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    let mut used_functions: Vec<String> = vec![];

    // Handle Nuxt event bus usage first (needs to be early for method transformations)
    if self.has_nuxt_event_bus(context) {
      used_functions.push("eventBus".to_string());
    }

    // Handle Nuxt redirect usage
    if self.has_nuxt_redirect(context) {
      used_functions.push("redirect".to_string());
    }

    // Handle Nuxt refresh usage
    if self.has_nuxt_refresh(context) {
      used_functions.push("refresh".to_string());
    }

    // Handle $config usage
    if self.has_config_usage(context) {
      used_functions.push("runtimeConfig".to_string());

      // Add template replacements for $config
      result
        .template_replacements
        .push(crate::TemplateReplacement {
          find: "$config".to_string(),
          replace: "runtimeConfig".to_string(),
        });
    }

    if !used_functions.is_empty() {
      result.add_import("@/composables/useNuxtCompat", "useNuxtCompat");
      let functions_list = used_functions.join(", ");
      result.setup.push(format!(
        "const {{ {} }} = useNuxtCompat();\n",
        functions_list
      ));
    }

    // Generate fetch method if it exists
    if self.has_fetch_method(context) {
      let fetch_code = self.generate_fetch_method(context, config);
      result.methods.extend(fetch_code);
    }

    // Generate asyncData method if it exists
    if self.has_async_data_method(context) {
      result.add_import("@/composables/useAsyncData", "useAsyncData");

      let async_data_code = self.generate_async_data_method(context, config);
      result.setup.extend(async_data_code);

      // Add high-priority data refs that override default data() refs
      let async_data_refs = self.generate_async_data_refs(context);
      result.data_refs.extend(async_data_refs);
    }

    // Generate i18n script block if nuxtI18n exists
    if self.has_nuxt_i18n(context) {
      // Add imports for i18n
      if let Some(i18n_script) = self.generate_i18n_script(context) {
        result.additional_scripts.push(i18n_script);
      }
    }

    // Handle nuxt-link to router-link transformation
    if self.has_nuxt_link_usage(context) {
      result
        .template_replacements
        .push(crate::TemplateReplacement {
          find: "nuxt-link".to_string(),
          replace: "router-link".to_string(),
        });
      result
        .template_replacements
        .push(crate::TemplateReplacement {
          find: "NuxtLink".to_string(),
          replace: "router-link".to_string(),
        });
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<BodyTransformFn>> {
    Some(Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let nuxt_transformer = NuxtTransformer::new();
        let mut transformed_body = body.to_string();

        // Transform this.$fetch() calls to fetch() calls
        if nuxt_transformer.has_fetch_calls(context) {
          transformed_body = transformed_body.replace("this.$fetch", "fetch");
          // Also handle cases where 'this.' was already removed by other transformations
          transformed_body = transformed_body.replace("$fetch", "fetch");
        }

        // Transform $nuxt event bus calls to eventBus calls
        if nuxt_transformer.has_nuxt_event_bus(context) {
          transformed_body = transformed_body.replace("this.$nuxt.$on", "eventBus.on");
          transformed_body = transformed_body.replace("this.$nuxt.$off", "eventBus.off");
          transformed_body = transformed_body.replace("this.$nuxt.$emit", "eventBus.emit");
        }

        // Transform $config usage in script
        if nuxt_transformer.has_config_usage(context) {
          transformed_body = transformed_body.replace("this.$config", "runtimeConfig");
          // Also handle cases where 'this.' was already removed by other transformations
          transformed_body = transformed_body.replace("$config", "runtimeConfig");
        }

        // Transform $nuxt.context.redirect usage in script
        if nuxt_transformer.has_nuxt_redirect(context) {
          transformed_body = transformed_body.replace("this.$nuxt.context.redirect", "redirect");
        }

        // Transform $nuxt.refresh usage in script
        if nuxt_transformer.has_nuxt_refresh(context) {
          transformed_body = transformed_body.replace("this.$nuxt.refresh", "refresh");
        }

        transformed_body
      },
    ))
  }
}
