use super::Transformer;
use crate::{TemplateReplacement, TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for converting Vue i18n usage from Options API to Composition API
///
/// This transformer handles the conversion of Vue i18n function calls ($t, $n, $d) from the
/// Options API pattern to the Composition API pattern using useI18n().
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::{
///     parse_sfc_sections, parse_script_section, parse_template_section,
///     ScriptParsingState, TemplateParsingState, TransformationContext, TransformerConfig
/// };
/// use vue_options_to_composition::transformers::{Transformer, i18n::I18nTransformer};
///
/// let sfc = r#"<template>
///     <h1>{{ $t('hello') }}</h1>
///     <span>{{ $n(count, 'currency') }}</span>
///     <span :title="$t('hello')">{{ $d(Date.now(), 'short') }}</span>
///     </template>
///     <script>
///     export default {
///       data() {
///         return {
///           count: 0,
///           somethingTranslated: this.$t('hello')
///         };
///       },
///       head() {
///         return {
///           title: this.$t('page.title')
///         };
///       },
///       methods: {
///         greet() {
///           return this.$t('hello');
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
/// let mut config = TransformerConfig::default();
/// config.enable_i18n = true;
///
/// // Test the transformer
/// let transformer = I18nTransformer::new();
///
/// // Should detect i18n usage
/// assert!(transformer.should_transform(&context, &config));
///
/// // Transform the component
/// let result = transformer.transform(&context, &config);
///
/// // Should replace template i18n calls
/// assert!(result.template_replacements.iter().any(|r| r.find == "$t(" && r.replace == "t("));
/// assert!(result.template_replacements.iter().any(|r| r.find == "$n(" && r.replace == "n("));
/// assert!(result.template_replacements.iter().any(|r| r.find == "$d(" && r.replace == "d("));
/// ```
pub struct I18nTransformer;

impl Default for I18nTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl I18nTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Apply i18n transformations to a body string
  pub fn apply_i18n_transforms(
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    if !config.enable_i18n {
      return body.to_string();
    }

    let mut result = body.to_string();

    // Transform i18n method calls
    result = result.replace("this.$t(", "t(");
    result = result.replace("this.$n(", "n(");
    result = result.replace("this.$d(", "d(");

    // Also handle cases where 'this.' was already removed by other transformations
    result = result.replace("$t(", "t(");
    result = result.replace("$n(", "n(");
    result = result.replace("$d(", "d(");

    // Transform i18n utils usage
    let i18n_transformer = I18nTransformer::new();
    if i18n_transformer.has_i18n_utils_usage(context) {
      // Transform $i18n.localeProperties to localeProperties
      result = result.replace("this.$i18n.localeProperties", "localeProperties");
      result = result.replace("$i18n.localeProperties", "localeProperties");
      result = result.replace("this.localePath", "localePath");
      result = result.replace("this.localeRoute", "localeRoute");
    }

    // Transform $i18n.locale usage
    if i18n_transformer.has_i18n_locale_usage(context) {
      // Transform this.$i18n.locale to locale.value
      result = result.replace("this.$i18n.locale", "locale.value");
      result = result.replace("$i18n.locale", "locale.value");
    }

    result
  }

  /// Check if i18n functions are used in the component
  fn has_i18n_usage(&self, context: &TransformationContext) -> bool {
    // Check function calls for $t, $n, $d (but not $set, $delete, etc.)
    context.script_state.function_calls.iter().any(|call| {
            call.contains("this.$t(") || call.contains("this.$n(") || call.contains("this.$d(") ||
            call.contains("$t(") || call.contains("$n(") || call.contains("$d(")
        }) ||
        // Check template for i18n usage
        context.template_state.function_calls.iter().any(|call| {
            call.contains("$t(") || call.contains("$n(") || call.contains("$d(")
        }) ||
        // Check identifiers
        context.script_state.identifiers.iter().any(|id| {
            id == "$t" || id == "$n" || id == "$d"
        }) ||
        context.template_state.identifiers.iter().any(|id| {
            id == "$t" || id == "$n" || id == "$d"
        })
  }

  /// Check if i18n utilities are used (localeProperties, localePath, localeRoute)
  fn has_i18n_utils_usage(&self, context: &TransformationContext) -> bool {
    // Check for $i18n.localeProperties usage
    let has_locale_properties = context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("$i18n.localeProperties"))
      || context.script_state.data_properties.iter().any(|prop| {
        prop
          .value
          .as_ref()
          .is_some_and(|v| v.contains("$i18n.localeProperties"))
      });

    // Check for localePath and localeRoute function calls
    let has_locale_functions = context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("localePath(") || call.contains("localeRoute("))
      || context
        .template_state
        .function_calls
        .iter()
        .any(|call| call.contains("localePath(") || call.contains("localeRoute("))
      || context
        .script_state
        .identifiers
        .iter()
        .any(|id| id == "localePath" || id == "localeRoute")
      || context
        .template_state
        .identifiers
        .iter()
        .any(|id| id == "localePath" || id == "localeRoute");

    has_locale_properties || has_locale_functions
  }

  /// Check if $i18n.locale is used in the component
  fn has_i18n_locale_usage(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .identifiers
      .iter()
      .any(|id| id == "$i18n.locale")
      || context
        .script_state
        .method_details
        .iter()
        .any(|method| method.body.contains("$i18n.locale"))
      || context.script_state.data_properties.iter().any(|prop| {
        prop
          .value
          .as_ref()
          .is_some_and(|v| v.contains("$i18n.locale"))
      })
  }

  /// Add i18n imports to the result
  fn add_i18n_imports(&self, result: &mut TransformationResult) {
    result.add_import("vue-i18n", "useI18n");
  }

  /// Add i18n utils imports to the result
  fn add_i18n_utils_imports(&self, result: &mut TransformationResult) {
    result.add_import("@/composables/useI18nUtils", "useI18nUtils");
  }

  /// Generate i18n utils setup code
  fn generate_i18n_utils_setup(&self, context: &TransformationContext) -> Vec<String> {
    let mut setup_code = Vec::new();
    let mut needed_utils = Vec::new();

    // Check what i18n utils are needed
    let has_locale_properties = context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("$i18n.localeProperties"))
      || context.script_state.data_properties.iter().any(|prop| {
        prop
          .value
          .as_ref()
          .is_some_and(|v| v.contains("$i18n.localeProperties"))
      });

    if has_locale_properties {
      needed_utils.push("localeProperties");
    }

    // Check for localePath and localeRoute
    let all_calls: Vec<&String> = context
      .script_state
      .function_calls
      .iter()
      .chain(context.template_state.function_calls.iter())
      .collect();
    let all_identifiers: Vec<&String> = context
      .script_state
      .identifiers
      .iter()
      .chain(context.template_state.identifiers.iter())
      .collect();

    if all_calls.iter().any(|call| call.contains("localePath("))
      || all_identifiers.iter().any(|id| *id == "localePath")
    {
      needed_utils.push("localePath");
    }

    if all_calls.iter().any(|call| call.contains("localeRoute("))
      || all_identifiers.iter().any(|id| *id == "localeRoute")
    {
      needed_utils.push("localeRoute");
    }

    if !needed_utils.is_empty() {
      let destructured = needed_utils.join(", ");
      setup_code.push(format!("const {{ {} }} = useI18nUtils();", destructured));
      setup_code.push("".to_string()); // Empty line for readability
    }

    setup_code
  }

  /// Generate i18n setup code
  fn generate_i18n_setup(&self, context: &TransformationContext) -> Vec<String> {
    let mut setup_code = Vec::new();
    let mut needed_functions = Vec::new();

    // Determine which i18n functions are needed
    let all_calls: Vec<&String> = context
      .script_state
      .function_calls
      .iter()
      .chain(context.template_state.function_calls.iter())
      .collect();
    let all_identifiers: Vec<&String> = context
      .script_state
      .identifiers
      .iter()
      .chain(context.template_state.identifiers.iter())
      .collect();
    let has_i18n_locale = self.has_i18n_locale_usage(context);

    if all_calls.iter().any(|call| call.contains("$t("))
      || all_identifiers.iter().any(|id| *id == "$t")
    {
      needed_functions.push("t");
    }
    if all_calls.iter().any(|call| call.contains("$n("))
      || all_identifiers.iter().any(|id| *id == "$n")
    {
      needed_functions.push("n");
    }
    if all_calls.iter().any(|call| call.contains("$d("))
      || all_identifiers.iter().any(|id| *id == "$d")
    {
      needed_functions.push("d");
    }

    if has_i18n_locale {
      needed_functions.push("locale");
    }

    if !needed_functions.is_empty() {
      let destructured = needed_functions.join(", ");
      setup_code.push(format!("const {{ {} }} = useI18n();", destructured));
      setup_code.push("".to_string()); // Empty line for readability
    }

    setup_code
  }

  /// Generate template replacements for i18n function calls
  fn generate_template_replacements(
    &self,
    context: &TransformationContext,
  ) -> Vec<TemplateReplacement> {
    let mut replacements = Vec::new();

    // Replace $t with t, $n with n, $d with d in templates
    replacements.push(TemplateReplacement {
      find: "$t(".to_string(),
      replace: "t(".to_string(),
    });
    replacements.push(TemplateReplacement {
      find: "$n(".to_string(),
      replace: "n(".to_string(),
    });
    replacements.push(TemplateReplacement {
      find: "$d(".to_string(),
      replace: "d(".to_string(),
    });

    // Replace $i18n.locale with locale in templates
    if self.has_i18n_locale_usage(context) {
      replacements.push(TemplateReplacement {
        find: "$i18n.locale".to_string(),
        replace: "locale".to_string(),
      });
    }

    replacements
  }
}

impl Transformer for I18nTransformer {
  fn name(&self) -> &'static str {
    "i18n"
  }

  fn should_transform(&self, context: &TransformationContext, config: &TransformerConfig) -> bool {
    config.enable_i18n
      && (self.has_i18n_usage(context)
        || self.has_i18n_utils_usage(context)
        || self.has_i18n_locale_usage(context))
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Handle standard i18n usage ($t, $n, $d) or locale usage
    if self.has_i18n_usage(context) || self.has_i18n_locale_usage(context) {
      // Add imports
      self.add_i18n_imports(&mut result);

      // Generate setup code for standard i18n
      result.setup.extend(self.generate_i18n_setup(context));

      // Generate replacements
      result
        .template_replacements
        .extend(self.generate_template_replacements(context));
    }

    // Handle i18n utils usage (localeProperties, localePath, localeRoute)
    if self.has_i18n_utils_usage(context) {
      self.add_i18n_utils_imports(&mut result);
      result.setup.extend(self.generate_i18n_utils_setup(context));
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<super::BodyTransformFn>> {
    Some(Box::new(|body, context, config| {
      Self::apply_i18n_transforms(body, context, config)
    }))
  }
}
