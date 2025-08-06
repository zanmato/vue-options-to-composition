use crate::{TransformationContext, TransformationResult, TransformerConfig};

// Sub-modules for different types of transformers
pub mod assets;
pub mod axios;
pub mod composition;
pub mod emit;
pub mod filters;
pub mod head;
pub mod i18n;
pub mod import_rewrite;
pub mod mixin;
pub mod nuxt;
pub mod router;
pub mod vue2;
pub mod vuex;

/// Function type for transforming method bodies
pub type BodyTransformFn = dyn Fn(&str, &TransformationContext, &TransformerConfig) -> String;

/// Trait that all transformers must implement
pub trait Transformer {
  /// Name of the transformer for logging/debugging
  fn name(&self) -> &'static str;

  /// Check if this transformer should run based on the context
  fn should_transform(&self, context: &TransformationContext, config: &TransformerConfig) -> bool;

  /// Perform the transformation
  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult;

  /// Dependencies on other transformers (by name)
  fn dependencies(&self) -> Vec<&'static str> {
    vec![]
  }

  /// Get body transformation function if this transformer provides one
  fn get_body_transform(&self) -> Option<Box<BodyTransformFn>> {
    None
  }
}

/// Utility functions for common body transformations
pub mod body_transforms {
  use crate::{TransformationContext, TransformerConfig};
  use lazy_static::lazy_static;
  use regex::Regex;

  lazy_static! {
      static ref THIS_PROPERTY_PATTERN: Regex = Regex::new(r"this\.([a-zA-Z_$][a-zA-Z0-9_$]*)").unwrap();
  }

  /// Apply reactive reference transformations to a body string
  pub fn apply_reactive_transforms(
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    let mut result = body.to_string();

    // Transform data property accesses (this.prop -> prop.value)
    // Sort by length (longest first) to prevent substring replacements
    let mut data_props_sorted = context.script_state.data_properties.clone();
    data_props_sorted.sort_by(|a, b| b.name.len().cmp(&a.name.len()));
    
    for data_prop in &data_props_sorted {
      let this_access = format!("this.{}", data_prop.name);
      let ref_access = format!("{}.value", data_prop.name);
      result = result.replace(&this_access, &ref_access);
    }

    // Transform computed property accesses (this.computed -> computed.value)
    // Sort by length (longest first) to prevent substring replacements
    let mut computed_props_sorted = context.script_state.computed_properties.clone();
    computed_props_sorted.sort_by(|a, b| b.len().cmp(&a.len()));
    
    for computed_prop in &computed_props_sorted {
      let this_access = format!("this.{}", computed_prop);
      let ref_access = format!("{}.value", computed_prop);
      result = result.replace(&this_access, &ref_access);
    }

    // Transform prop accesses (this.propName -> props.propName)
    // Sort by length (longest first) to prevent substring replacements
    let mut props_sorted = context.script_state.props.clone();
    props_sorted.sort_by(|a, b| b.name.len().cmp(&a.name.len()));
    
    for prop in &props_sorted {
      let this_access = format!("this.{}", prop.name);
      let prop_access = format!("props.{}", prop.name);
      result = result.replace(&this_access, &prop_access);
    }

    // Transform method calls (this.method() -> method())
    // Sort by length (longest first) to prevent substring replacements
    let mut methods_sorted = context.script_state.methods.clone();
    methods_sorted.sort_by(|a, b| b.len().cmp(&a.len()));
    
    for method in &methods_sorted {
      let this_call = format!("this.{}(", method);
      let direct_call = format!("{}(", method);
      result = result.replace(&this_call, &direct_call);
    }

    // Transform method references (this.method -> method)
    // This handles method references (not calls) like in event listeners
    for method in &methods_sorted {
      let this_ref = format!("this.{}", method);
      let direct_ref = method;

      // Use simple replace for method references (not followed by parentheses)
      // This handles cases like: this.handleEvent -> handleEvent
      if result.contains(&this_ref) {
        // Only replace if it's not a method call (not followed by '(')
        let this_call = format!("this.{}(", method);
        if !result.contains(&this_call) {
          result = result.replace(&this_ref, direct_ref);
        }
      }
    }

    // Handle any remaining this.* references that weren't transformed
    // These are likely non-existent variables that should get FIXME comments
    {
      let re = &*THIS_PROPERTY_PATTERN;
      result = re
        .replace_all(&result, |caps: &regex::Captures| {
          let var_name = &caps[1];

          // Check if this variable exists in any known scope
          let exists_in_data = context
            .script_state
            .data_properties
            .iter()
            .any(|prop| prop.name == var_name);
          let exists_in_computed = context
            .script_state
            .computed_properties
            .iter()
            .any(|prop| prop == var_name);
          let exists_in_props = context
            .script_state
            .props
            .iter()
            .any(|prop| prop.name == var_name);
          let exists_in_methods = context
            .script_state
            .methods
            .iter()
            .any(|method| method == var_name);

          // Check if this is a known Vue/framework variable that should be handled by transformers
          let is_framework_variable = matches!(
            var_name,
            "$axios"
              | "$http"
              | "$api"
              | "$fetch"
              | "$route"
              | "$router"
              | "$store"
              | "$vuex"
              | "$t"
              | "$i18n"
              | "$n"
              | "$d"
              | "$config"
              | "$nextTick"
              | "$refs"
              | "$emit"
              | "$nuxt"
              | "$options"
              | "$parent"
              | "$children"
              | "$el"
              | "$data"
              | "$props"
              | "$attrs"
              | "$slots"
              | "$scopedSlots"
              | "$set"
              | "$delete"
              | "$watch"
              | "$forceUpdate"
              | "$destroy"
          );

          // Check if this property is provided by a mixin composable
          let is_mixin_property = if let Some(mixin_configs) = &config.mixins {
            mixin_configs.values().any(|mixin_config| {
              mixin_config.imports.contains(&var_name.to_string())
            })
          } else {
            false
          };

          if exists_in_data || exists_in_computed || exists_in_props || exists_in_methods {
            // This should have been transformed by earlier logic, but wasn't
            // Just remove the 'this.' for now
            var_name.to_string()
          } else if is_framework_variable || is_mixin_property {
            // This is a framework variable or mixin property that should be handled by a transformer
            // but apparently wasn't - don't add FIXME, just remove 'this.'
            var_name.to_string()
          } else {
            // This variable doesn't exist in the component and isn't a known framework variable
            // Add FIXME comment
            format!("/* FIXME: {} */ {}", var_name, var_name)
          }
        })
        .to_string();
    }

    result
  }

  /// Apply all common body transformations
  pub fn apply_all_body_transforms(
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
    additional_transforms: &[Box<super::BodyTransformFn>],
  ) -> String {
    let mut transformed_body = body.to_string();

    // Apply additional transforms from transformers (including i18n)
    for transform_fn in additional_transforms {
      transformed_body = transform_fn(&transformed_body, context, config);
    }

    // Apply built-in reactive transforms
    transformed_body = apply_reactive_transforms(&transformed_body, context, config);

    transformed_body
  }
}

/// Main transformer orchestrator that runs all transformers
pub struct TransformerOrchestrator {
  transformers: Vec<Box<dyn Transformer>>,
}

impl Default for TransformerOrchestrator {
  fn default() -> Self {
    Self::new()
  }
}

impl TransformerOrchestrator {
  pub fn new() -> Self {
    let transformers: Vec<Box<dyn Transformer>> = vec![
      Box::new(axios::AxiosTransformer::new()),
      Box::new(import_rewrite::ImportRewriteTransformer::new()),
      Box::new(mixin::MixinTransformer::new()),
      Box::new(nuxt::NuxtTransformer::new()),
      Box::new(router::RouterTransformer::new()),
      Box::new(vue2::Vue2Transformer::new()),
      Box::new(filters::FiltersTransformer::new()),
      Box::new(vuex::VuexTransformer::new()),
      Box::new(composition::CompositionTransformer::new()),
      Box::new(emit::EmitTransformer::new()),
      Box::new(i18n::I18nTransformer::new()),
      Box::new(head::HeadTransformer::new()),
      Box::new(assets::AssetsTransformer::new()),
    ];

    Self { transformers }
  }

  /// Collect all body transformation functions from transformers
  pub fn collect_body_transforms(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<Box<BodyTransformFn>> {
    self
      .transformers
      .iter()
      .filter(|t| t.should_transform(context, config))
      .filter_map(|t| t.get_body_transform())
      .collect()
  }

  /// Apply all available body transformations to a method body
  pub fn transform_method_body(
    &self,
    body: &str,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> String {
    let additional_transforms = self.collect_body_transforms(context, config);
    body_transforms::apply_all_body_transforms(body, context, config, &additional_transforms)
  }

  /// Get a static method for transforming bodies (for use in transformers)
  pub fn get_body_transformer(
  ) -> impl Fn(&str, &TransformationContext, &TransformerConfig) -> String {
    |body: &str, context: &TransformationContext, config: &TransformerConfig| {
      // Create a temporary orchestrator to get body transforms
      let orchestrator = TransformerOrchestrator::new();
      let additional_transforms = orchestrator.collect_body_transforms(context, config);
      body_transforms::apply_all_body_transforms(body, context, config, &additional_transforms)
    }
  }

  /// Transform a Vue component using all applicable transformers
  pub fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Filter transformers that should run
    let applicable_transformers: Vec<&Box<dyn Transformer>> = self
      .transformers
      .iter()
      .filter(|t| t.should_transform(context, config))
      .collect();

    // Apply each transformer and collect results
    let mut all_results = Vec::new();
    for transformer in applicable_transformers {
      let transformer_result = transformer.transform(context, config);
      all_results.push((transformer.name(), transformer_result));
    }

    // Merge results with intelligent ordering
    self.merge_results_intelligently(&mut result, all_results);

    result
  }

  /// Merge results with intelligent ordering using the new structured approach
  fn merge_results_intelligently(
    &self,
    result: &mut TransformationResult,
    all_results: Vec<(&'static str, TransformationResult)>,
  ) {
    let mut imports_to_remove = Vec::new();
    let mut has_computed_from_transformers = false;

    // Merge all structured results directly
    for (_transformer_name, transformer_result) in all_results {
      // Check if this transformer produced computed properties (before merging)
      if !transformer_result.computed_properties.is_empty() {
        has_computed_from_transformers = true;
      }

      // Merge imports
      for (path, imports) in transformer_result.imports_to_add {
        result
          .imports_to_add
          .entry(path)
          .or_default()
          .extend(imports);
      }
      imports_to_remove.extend(transformer_result.imports_to_remove);

      // Merge structured setup code directly in the right order
      result.setup.extend(transformer_result.setup);
      result
        .reactive_state
        .extend(transformer_result.reactive_state);
      result
        .computed_properties
        .extend(transformer_result.computed_properties);
      result.methods.extend(transformer_result.methods);
      result.watchers.extend(transformer_result.watchers);
      result
        .lifecycle_hooks
        .extend(transformer_result.lifecycle_hooks);

      // Merge other fields
      result
        .template_replacements
        .extend(transformer_result.template_replacements);
      result
        .additional_scripts
        .extend(transformer_result.additional_scripts);
      result
        .skip_data_properties
        .extend(transformer_result.skip_data_properties);

      // Merge data refs with priority
      for (prop_name, (ref_declaration, priority)) in transformer_result.data_refs {
        match result.data_refs.get(&prop_name) {
          Some((_, existing_priority)) if *existing_priority >= priority => {
            // Keep existing ref if it has higher or equal priority
          }
          _ => {
            // Override with new ref (higher priority or first time)
            result
              .data_refs
              .insert(prop_name, (ref_declaration, priority));
          }
        }
      }
    }

    // Remove imports that should be filtered out
    for package_to_remove in &imports_to_remove {
      result.imports_to_add.remove(package_to_remove);
    }

    // Process data_refs into reactive_state with proper priority ordering
    let mut data_refs_sorted: Vec<_> = result.data_refs.iter().collect();
    data_refs_sorted.sort_by(|a, b| {
      // Sort by priority (higher first), then by name for deterministic output
      b.1 .1.cmp(&a.1 .1).then_with(|| a.0.cmp(b.0))
    });

    // Add data refs to reactive_state
    for (_, (ref_declaration, _)) in data_refs_sorted {
      result.reactive_state.push(ref_declaration.clone());
    }

    // If we have computed properties from transformers, ensure we have the computed import
    if has_computed_from_transformers {
      let vue_imports = result.imports_to_add.entry("vue".to_string()).or_default();
      if !vue_imports.contains(&"computed".to_string()) {
        vue_imports.push("computed".to_string());
      }
    }
  }

  /// Get a list of all available transformer names
  pub fn available_transformers(&self) -> Vec<&'static str> {
    self.transformers.iter().map(|t| t.name()).collect()
  }
}
