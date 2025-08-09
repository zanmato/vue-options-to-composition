use super::Transformer;
use super::TransformerOrchestrator;
use crate::{TemplateReplacement, TransformationContext, TransformationResult, TransformerConfig};
use std::collections::HashMap;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ASYNC_COMPONENT_DETECTION_PATTERN: Regex = Regex::new(r"const\s+\w+\s*=\s*\(\s*\)\s*=>\s*import\s*\(").unwrap();
    static ref ASYNC_COMPONENT_TRANSFORM_PATTERN: Regex = Regex::new(r"(?s)const\s+(\w+)\s*=\s*\(\s*\)\s*=>\s*import\s*\(([^)]+)\)").unwrap();
}

/// Transformer for converting Options API to Composition API
///
/// This transformer handles the conversion from Vue Options API to Composition API by:
/// - Converting `props` to `defineProps()` declarations
/// - Converting `data()` properties to `ref()` declarations
/// - Converting `computed` properties to `computed()` functions
/// - Converting `methods` to regular functions
/// - Generating appropriate Vue 3 imports
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::{
///     parse_sfc_sections, parse_script_section, parse_template_section,
///     ScriptParsingState, TemplateParsingState, TransformationContext, TransformerConfig
/// };
/// use vue_options_to_composition::transformers::{Transformer, composition::CompositionTransformer};
///
/// let sfc = r#"<template><h1>{{ count }}</h1></template>
///     <script>
///     export default {
///       data() {
///         return {
///           count: 0
///         };
///       },
///       methods: {
///         increment() {
///           this.count++;
///         }
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
/// let transformer = CompositionTransformer::new();
///
/// // Should detect data properties and methods to transform
/// assert!(transformer.should_transform(&context, &config));
///
/// // Transform the component
/// let result = transformer.transform(&context, &config);
///
/// // Should generate ref declarations for data properties in data_refs
/// assert!(result.data_refs.contains_key("count"));
/// assert!(result.data_refs.get("count").unwrap().0.contains("const count = ref(0);"));
pub struct CompositionTransformer;

impl Default for CompositionTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositionTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Generate Vue imports for data properties, computed properties, and watchers
  fn generate_vue_imports(&self, context: &TransformationContext) -> Vec<String> {
    let mut vue_imports = Vec::new();

    // Collect needed Vue imports
    if !context.script_state.data_properties.is_empty() {
      vue_imports.push("ref".to_string());
    }

    if !context.script_state.computed_details.is_empty()
      || !context.script_state.computed_properties.is_empty()
    {
      vue_imports.push("computed".to_string());
    }

    if !context.script_state.watchers.is_empty() {
      vue_imports.push("watch".to_string());
    }

    // Check if async components are used and add defineAsyncComponent
    if self.has_async_components(context) {
      vue_imports.push("defineAsyncComponent".to_string());
    }

    // Return Vue imports as vector for adding to result
    vue_imports
  }

  /// Generate setup code for props using defineProps
  fn generate_props_definition(&self, context: &TransformationContext) -> Vec<String> {
    let mut setup_code = Vec::new();

    if !context.script_state.props.is_empty() {
      setup_code.push("const props = defineProps({".to_string());

      for prop in &context.script_state.props {
        setup_code.push(format!("  {}: {{", prop.name));

        if let Some(prop_type) = &prop.prop_type {
          setup_code.push(format!("    type: {},", prop_type));
        }

        if let Some(required) = prop.required {
          setup_code.push(format!("    required: {},", required));
        }

        if let Some(default_value) = &prop.default_value {
          setup_code.push(format!("    default: {},", default_value));
        }

        setup_code.push("  },".to_string());
      }

      setup_code.push("});".to_string());
      setup_code.push("".to_string()); // Empty line for readability
    }

    setup_code
  }

  /// Generate setup code for data properties as refs
  fn generate_data_refs(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> HashMap<String, (String, u8)> {
    use std::collections::HashMap;
    let mut data_refs = HashMap::new();

    for data_prop in &context.script_state.data_properties {
      let initial_value = data_prop.value.as_deref().unwrap_or("undefined");

      // Apply transformations to the initial value
      let transformed_value = self.transform_data_value(initial_value, context, config);

      let ref_declaration = format!("const {} = ref({});", data_prop.name, transformed_value);

      // Use priority 0 for default data() refs (can be overridden by other transformers)
      data_refs.insert(data_prop.name.clone(), (ref_declaration, 0));
    }

    data_refs
  }

  /// Transform data property initial values
  fn transform_data_value(
    &self,
    value: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    // Use the orchestrator's body transformer which applies all available transforms
    let body_transformer = TransformerOrchestrator::get_body_transformer();
    body_transformer(value, context, config)
  }

  /// Generate setup code for computed properties
  fn generate_computed_properties(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<String> {
    let mut setup_code = Vec::new();

    if !context.script_state.computed_details.is_empty() {
      for computed_detail in &context.script_state.computed_details {
        // Generate computed property based on whether it has getter/setter
        if let (Some(getter), Some(setter)) = (&computed_detail.getter, &computed_detail.setter) {
          // Computed with getter and setter
          setup_code.push(format!("const {} = computed({{", computed_detail.name));
          setup_code.push("  get() {".to_string());

          // Transform the getter body
          let transformed_getter = self.transform_computed_body(getter, context, config);
          for line in transformed_getter.lines() {
            if !line.trim().is_empty() {
              setup_code.push(format!("    {}", line));
            }
          }

          setup_code.push("  },".to_string());
          
          // Use the original setter parameter name if available, otherwise default to "v"
          let setter_param = computed_detail.setter_parameter.as_deref().unwrap_or("v");
          setup_code.push(format!("  set({}) {{", setter_param));

          // Transform the setter body
          let transformed_setter = self.transform_computed_body(setter, context, config);
          for line in transformed_setter.lines() {
            if !line.trim().is_empty() {
              setup_code.push(format!("    {}", line));
            }
          }

          setup_code.push("  },".to_string());
          setup_code.push("});".to_string());
        } else if let Some(getter) = &computed_detail.getter {
          // Computed with getter only
          setup_code.push(format!(
            "const {} = computed(() => {{",
            computed_detail.name
          ));

          // Transform the getter body
          let transformed_getter = self.transform_computed_body(getter, context, config);
          for line in transformed_getter.lines() {
            if !line.trim().is_empty() {
              setup_code.push(format!("  {}", line));
            }
          }

          setup_code.push("});".to_string());
        } else {
          // Fallback for computed properties without details
          setup_code.push(format!(
            "const {} = computed(() => {{",
            computed_detail.name
          ));
          setup_code.push("  // TODO: Implement computed logic".to_string());
          setup_code.push("  return undefined;".to_string());
          setup_code.push("});".to_string());
        }
      }

      if !setup_code.is_empty() {
        setup_code.push("".to_string()); // Empty line for readability
      }
    }

    setup_code
  }

  /// Transform computed property body by applying other transformers
  fn transform_computed_body(
    &self,
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    // Use the orchestrator's body transformer which applies all available transforms
    let body_transformer = TransformerOrchestrator::get_body_transformer();
    body_transformer(body, context, config)
  }

  /// Generate setup code for methods
  fn generate_methods(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<String> {
    let mut setup_code = Vec::new();

    // Use method_details if available, otherwise fall back to method names
    if !context.script_state.method_details.is_empty() {
      for method_detail in &context.script_state.method_details {
        // Skip lifecycle methods that are handled by other transformers (like Vue2 transformer)
        if matches!(
          method_detail.name.as_str(),
          "beforeCreate"
            | "created"
            | "beforeMount"
            | "mounted"
            | "beforeUpdate"
            | "updated"
            | "beforeDestroy"
            | "destroyed"
            | "beforeUnmount"
            | "unmounted"
            | "activated"
            | "deactivated"
        ) {
          continue;
        }

        let async_part = if method_detail.is_async { "async " } else { "" };
        let params_str = method_detail.parameters.join(", ");

        // Transform the method body using other transformers
        let transformed_body = self.transform_method_body(&method_detail.body, context, config);

        setup_code.push(format!(
          "const {} = {}({}) => {{",
          method_detail.name, async_part, params_str
        ));

        // Add the transformed body with proper indentation
        for line in transformed_body.lines() {
          if !line.trim().is_empty() {
            setup_code.push(format!("  {}", line));
          }
        }

        setup_code.push("};".to_string());
      }
    } else if !context.script_state.methods.is_empty() {
      // Fallback for backward compatibility
      for method in &context.script_state.methods {
        // Skip lifecycle methods
        if matches!(
          method.as_str(),
          "mounted" | "created" | "beforeDestroy" | "activated" | "deactivated"
        ) {
          continue;
        }

        setup_code.push(format!("const {} = () => {{", method));
        setup_code.push("  // TODO: Method body not parsed".to_string());
        setup_code.push("};".to_string());
      }
    }

    setup_code
  }

  /// Transform method body by applying other transformers
  fn transform_method_body(
    &self,
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    // Use the orchestrator's body transformer which applies all available transforms
    let body_transformer = TransformerOrchestrator::get_body_transformer();
    body_transformer(body, context, config)
  }

  /// Generate existing imports that are not handled by other transformers
  fn generate_existing_imports(&self, context: &TransformationContext) -> Vec<String> {
    // Generate imports from parsed import information, but only for simple imports
    // that don't have special handling (no mixins, no bootstrap-vue, etc.)
    let mut imports = Vec::new();

    for import_info in &context.script_state.imports {
      // Skip imports that are likely handled by other transformers
      if import_info.source.contains("@/mixins/")
        || import_info.source.contains("bootstrap-vue")
        || import_info.source.contains("@/composables/")
        || import_info.source == "vuex"
      {
        continue;
      }

      // Generate simple imports from relative paths or library imports
      let mut import_parts = Vec::new();

      for import_item in &import_info.imports {
        if import_item.is_default {
          import_parts.push(import_item.name.clone());
        } else if import_item.is_namespace {
          import_parts.push(format!("* as {}", import_item.name));
        } else if let Some(alias) = &import_item.alias {
          import_parts.push(format!("{} as {}", import_item.name, alias));
        } else {
          import_parts.push(import_item.name.clone());
        }
      }

      if !import_parts.is_empty() {
        // Check if we have default imports
        let has_default = import_info.imports.iter().any(|item| item.is_default);
        let has_named = import_info
          .imports
          .iter()
          .any(|item| !item.is_default && !item.is_namespace);

        let import_statement = if has_default && !has_named {
          // Only default import(s) - no braces needed
          format!(
            "import {} from '{}';",
            import_parts.join(", "),
            import_info.source
          )
        } else if !has_default && has_named {
          // Only named imports - use braces
          format!(
            "import {{ {} }} from '{}';",
            import_parts.join(", "),
            import_info.source
          )
        } else {
          // Mixed imports (default + named) - format properly
          let default_parts: Vec<String> = import_info
            .imports
            .iter()
            .filter(|item| item.is_default)
            .map(|item| item.name.clone())
            .collect();
          let named_parts: Vec<String> = import_info
            .imports
            .iter()
            .filter(|item| !item.is_default && !item.is_namespace)
            .map(|item| {
              if let Some(alias) = &item.alias {
                format!("{} as {}", item.name, alias)
              } else {
                item.name.clone()
              }
            })
            .collect();

          if !default_parts.is_empty() && !named_parts.is_empty() {
            format!(
              "import {}, {{ {} }} from '{}';",
              default_parts.join(", "),
              named_parts.join(", "),
              import_info.source
            )
          } else {
            // Fallback to the original logic
            format!(
              "import {{ {} }} from '{}';",
              import_parts.join(", "),
              import_info.source
            )
          }
        };
        imports.push(import_statement);
      }
    }

    imports
  }

  /// Check if the context contains async component definitions
  fn has_async_components(&self, context: &TransformationContext) -> bool {
    if let Some(setup_content) = &context.script_state.setup_content {
      // Look for dynamic import patterns: const ComponentName = () => import(...)
      ASYNC_COMPONENT_DETECTION_PATTERN.is_match(setup_content)
    } else {
      false
    }
  }

  /// Generate setup content (constants and other code between imports and export)
  fn generate_setup_content(&self, context: &TransformationContext) -> Vec<String> {
    if let Some(setup_content) = &context.script_state.setup_content {
      // First, transform async components in the entire content to handle multi-line declarations
      let transformed_content = ASYNC_COMPONENT_TRANSFORM_PATTERN.replace_all(&setup_content, "const $1 = defineAsyncComponent(() => import($2))");
      
      // Extract everything except import statements from setup content
      let mut result = Vec::new();
      for line in transformed_content.lines() {
        if !line.trim().starts_with("import ") && !line.trim().is_empty() {
          result.push(line.to_string());
        }
      }

      if !result.is_empty() {
        result.push("".to_string()); // Add empty line for readability
      }

      result
    } else {
      vec![]
    }
  }

  /// Generate setup code for watchers
  fn generate_watchers(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<String> {
    let mut setup_code = Vec::new();

    for watcher in &context.script_state.watchers {
      // Transform the watcher body using other transformers
      let transformed_body = self.transform_watcher_body(&watcher.handler_body, context, config);

      // Generate watch call
      let async_part = if watcher.is_async { "async " } else { "" };
      setup_code.push(format!(
        "watch({}, {}({}, {}) => {{",
        watcher.watched_property, async_part, watcher.param_names.0, watcher.param_names.1
      ));

      // Add the transformed body with proper indentation
      for line in transformed_body.lines() {
        if !line.trim().is_empty() {
          setup_code.push(format!("  {}", line));
        }
      }

      setup_code.push("});".to_string());
    }

    if !context.script_state.watchers.is_empty() {
      setup_code.push("".to_string()); // Empty line for readability
    }

    setup_code
  }

  /// Transform watcher body by applying other transformers
  fn transform_watcher_body(
    &self,
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    // Use the orchestrator's body transformer which applies all available transforms
    let body_transformer = TransformerOrchestrator::get_body_transformer();
    body_transformer(body, context, config)
  }

  /// Generate return statement for setup function
  fn generate_return_statement(&self, _context: &TransformationContext) -> Vec<String> {
    // In <script setup>, we don't need a return statement
    // The variables are automatically exposed to the template
    vec![]
  }

  /// Generate template replacements for reactive references
  fn generate_template_replacements(
    &self,
    _context: &TransformationContext,
  ) -> Vec<TemplateReplacement> {
    

    // In Composition API, we don't need .value in templates, so no replacements needed
    // The template syntax remains the same

    Vec::new()
  }
}

impl Transformer for CompositionTransformer {
  fn name(&self) -> &'static str {
    "composition"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    // Transform if we have props, data properties, computed properties, methods, watchers, lifecycle methods, or setup content
    !context.script_state.props.is_empty()
      || !context.script_state.data_properties.is_empty()
      || !context.script_state.computed_details.is_empty()
      || !context.script_state.methods.is_empty()
      || !context.script_state.method_details.is_empty()
      || !context.script_state.watchers.is_empty()
      || context.script_state.setup_content.is_some()
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Generate imports (including existing imports from setup_content)
    for import in self.generate_existing_imports(context) {
      // Parse and add existing imports (these are already formatted)
      // For now, keep them as-is in the old format - this could be improved later
      result
        .imports_to_add
        .entry("__existing__".to_string())
        .or_default()
        .push(import);
    }

    // Add Vue imports
    let vue_imports = self.generate_vue_imports(context);
    if !vue_imports.is_empty() {
      result.add_imports(
        "vue",
        &vue_imports.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
      );
    }

    // Generate setup code - existing content and defineProps
    result.setup.extend(self.generate_setup_content(context));
    result.setup.extend(self.generate_props_definition(context));

    // Add data refs to the reactive_state
    let data_refs = self.generate_data_refs(context, config);
    result.data_refs.extend(data_refs);

    // Add computed properties
    result
      .computed_properties
      .extend(self.generate_computed_properties(context, config));

    // Add methods
    result
      .methods
      .extend(self.generate_methods(context, config));

    // Add watchers
    result
      .watchers
      .extend(self.generate_watchers(context, config));

    // Add return statement to setup section
    result.setup.extend(self.generate_return_statement(context));

    // Generate replacements
    result
      .template_replacements
      .extend(self.generate_template_replacements(context));

    result
  }
}
