use super::{Transformer, TransformerOrchestrator};
use crate::{TransformationContext, TransformationResult, TransformerConfig};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref VUE2_SET_THIS_PATTERN: Regex = Regex::new(r"this\.\$set\(this\.(\w+),\s*([^,]+),\s*([^)]+)\)").unwrap();
    static ref VUE2_SET_VALUE_PATTERN: Regex = Regex::new(r"this\.\$set\((\w+)\.value,\s*([^,]+),\s*([^)]+)\)").unwrap();
    static ref VUE2_DELETE_PATTERN: Regex = Regex::new(r"this\.\$delete\(this\.(\w+),\s*([^)]+)\)").unwrap();
    static ref VUE2_REFS_DOT_PATTERN: Regex = Regex::new(r"\$refs\.([a-zA-Z_$][a-zA-Z0-9_$]*)").unwrap();
    static ref VUE2_REFS_BRACKET_PATTERN: Regex = Regex::new(r#"\$refs\[['"]([^'"]+)['"]\]"#).unwrap();
}

/// Transformer for Vue 2 specific methods that need to be converted for Vue 3
///
/// This transformer handles:
/// - Converting `this.$set(obj, key, value)` to `obj.value[key] = value`
/// - Converting `this.$delete(obj, key)` to `delete obj.value[key]`
/// - Adding `onMounted` import when needed
pub struct Vue2Transformer;

impl Default for Vue2Transformer {
    fn default() -> Self {
        Self::new()
    }
}

impl Vue2Transformer {
  pub fn new() -> Self {
    Self
  }

  /// Get body transformation function for converting $set and $delete calls
  fn get_vue2_body_transform(
  ) -> Box<dyn Fn(&str, &TransformationContext, &TransformerConfig) -> String> {
    Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let mut transformed_body = body.to_string();

        // Transform $set calls: this.$set(this.obj, key, value) -> obj.value[key] = value
        // Also handle already transformed: this.$set(obj.value, key, value) -> obj.value[key] = value
        let set_pattern1 = &*VUE2_SET_THIS_PATTERN;
        transformed_body = set_pattern1
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let obj_name = &caps[1];
            let key = &caps[2];
            let value = &caps[3];

            // Check if the key is a string literal or a template literal
            if key.starts_with("'") || key.starts_with("\"") {
              // String literal key: obj.value.key = value
              let clean_key = key.trim_matches(|c| c == '\'' || c == '"');
              format!("{}.value.{} = {}", obj_name, clean_key, value)
            } else {
              // Dynamic key: obj.value[key] = value
              format!("{}.value[{}] = {}", obj_name, key, value)
            }
          })
          .to_string();

        // Handle already transformed reactive references: this.$set(obj.value, key, value) -> obj.value[key] = value
        let set_pattern2 = &*VUE2_SET_VALUE_PATTERN;
        transformed_body = set_pattern2
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let obj_name = &caps[1];
            let key = &caps[2];
            let value = &caps[3];

            // Check if the key is a string literal or a template literal
            if key.starts_with("'") || key.starts_with("\"") {
              // String literal key: obj.value.key = value
              let clean_key = key.trim_matches(|c| c == '\'' || c == '"');
              format!("{}.value.{} = {}", obj_name, clean_key, value)
            } else {
              // Dynamic key: obj.value[key] = value
              format!("{}.value[{}] = {}", obj_name, key, value)
            }
          })
          .to_string();

        // Transform $delete calls: this.$delete(this.obj, key) -> delete obj.value[key]
        let delete_pattern1 = &*VUE2_DELETE_PATTERN;
        transformed_body = delete_pattern1
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let obj_name = &caps[1];
            let key = &caps[2];

            // Check if the key is a string literal or a template literal
            if key.starts_with("'") || key.starts_with("\"") {
              // String literal key: delete obj.value.key
              let clean_key = key.trim_matches(|c| c == '\'' || c == '"');
              format!("delete {}.value.{}", obj_name, clean_key)
            } else {
              // Dynamic key: delete obj.value[key]
              format!("delete {}.value[{}]", obj_name, key)
            }
          })
          .to_string();

        // Transform $nextTick calls: this.$nextTick(...) -> nextTick(...)
        transformed_body = transformed_body.replace("this.$nextTick(", "nextTick(");

        // Also handle cases where 'this.' was already removed by other transformations
        transformed_body = transformed_body.replace("$nextTick(", "nextTick(");

        // Transform $refs usage: this.$refs.name -> nameRef.value and this.$refs['name'] -> nameRef.value
        let template_refs = Vue2Transformer::extract_template_refs(context);
        for ref_name in &template_refs {
          let var_name = Vue2Transformer::ref_name_to_variable(ref_name);

          // Transform dot notation: this.$refs.name -> nameRef.value
          let this_refs_pattern = format!("this.$refs.{}", ref_name);
          let var_access = format!("{}.value", var_name);
          transformed_body = transformed_body.replace(&this_refs_pattern, &var_access);

          // Transform optional chaining: this.$refs?.name -> nameRef.value
          let this_refs_optional_pattern = format!("this.$refs?.{}", ref_name);
          transformed_body = transformed_body.replace(&this_refs_optional_pattern, &var_access);

          // Transform bracket notation: this.$refs['name'] or this.$refs["name"] -> nameRef.value
          let bracket_pattern1 = format!("this.$refs['{}']", ref_name);
          let bracket_pattern2 = format!("this.$refs[\"{}\"]", ref_name);
          transformed_body = transformed_body.replace(&bracket_pattern1, &var_access);
          transformed_body = transformed_body.replace(&bracket_pattern2, &var_access);

          // Also handle cases where 'this.' was already removed by other transformations
          let refs_pattern = format!("$refs.{}", ref_name);
          let refs_optional_pattern = format!("$refs?.{}", ref_name);
          let refs_bracket1 = format!("$refs['{}']", ref_name);
          let refs_bracket2 = format!("$refs[\"{}\"]", ref_name);
          transformed_body = transformed_body.replace(&refs_pattern, &var_access);
          transformed_body = transformed_body.replace(&refs_optional_pattern, &var_access);
          transformed_body = transformed_body.replace(&refs_bracket1, &var_access);
          transformed_body = transformed_body.replace(&refs_bracket2, &var_access);
        }

        transformed_body
      },
    )
  }

  /// Check if the body contains Vue 2 reactivity methods
  fn has_vue2_methods(body: &str) -> bool {
    body.contains("$set(") || body.contains("$delete(") || body.contains("$nextTick(")
  }

  /// Check if the body contains $refs usage
  fn has_refs_usage(body: &str) -> bool {
    body.contains("this.$refs") || body.contains("$refs")
  }

  /// Check if any method in the context uses Vue 2 methods or refs
  fn context_has_vue2_methods(context: &TransformationContext) -> bool {
    // Check method details for Vue 2 methods (includes lifecycle methods like mounted)
    for method_detail in &context.script_state.method_details {
      // Check for Vue 2 specific methods ($set, $delete, $refs)
      if Self::has_vue2_methods(&method_detail.body) || Self::has_refs_usage(&method_detail.body) {
        return true;
      }

      // Also check for lifecycle methods that need transformation
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
        return true;
      }
    }

    // Also check if we have template refs that need to be set up
    Self::context_has_template_refs(context)
  }

  /// Check if the context has template refs that need to be handled
  fn context_has_template_refs(context: &TransformationContext) -> bool {
    // Check if there are any ref attributes in the template
    // This will be detected from template parsing
    !Self::extract_template_refs(context).is_empty()
  }

  /// Extract template ref names from the template context
  fn extract_template_refs(context: &TransformationContext) -> Vec<String> {
    // For now, we'll look for ref usage in method bodies and extract from there
    // A more complete implementation would parse the template directly
    let mut refs = Vec::new();

    for method_detail in &context.script_state.method_details {
      // Look for patterns like this.$refs.name or this.$refs['name']
      {
        let re = &*VUE2_REFS_DOT_PATTERN;
        for cap in re.captures_iter(&method_detail.body) {
          let ref_name = cap[1].to_string();
          if !refs.contains(&ref_name) {
            refs.push(ref_name);
          }
        }
      }

      // Look for bracket notation like this.$refs['name'] or this.$refs["name"]
      {
        let re = &*VUE2_REFS_BRACKET_PATTERN;
        for cap in re.captures_iter(&method_detail.body) {
          let ref_name = cap[1].to_string();
          if !refs.contains(&ref_name) {
            refs.push(ref_name);
          }
        }
      }
    }

    refs
  }

  /// Convert a ref name to a valid variable name with Ref suffix
  fn ref_name_to_variable(ref_name: &str) -> String {
    // Convert kebab-case to camelCase
    let camel_case = ref_name
      .split('-')
      .enumerate()
      .map(|(i, word)| {
        if i == 0 {
          word.to_string()
        } else {
          let mut chars = word.chars();
          match chars.next() {
            None => String::new(),
            Some(first) => {
              first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
            }
          }
        }
      })
      .collect::<String>();

    // Add Ref suffix if not already present
    if camel_case.ends_with("Ref") {
      camel_case
    } else {
      format!("{}Ref", camel_case)
    }
  }
}

impl Transformer for Vue2Transformer {
  fn name(&self) -> &'static str {
    "vue2"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    Self::context_has_vue2_methods(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Check if we have lifecycle methods that need Vue imports
    let has_lifecycle_methods = context.script_state.method_details.iter().any(|method| {
      matches!(
        method.name.as_str(),
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
      )
    });

    if has_lifecycle_methods {
      // Add Vue lifecycle imports
      let mut vue_imports = vec![];

      for method_detail in &context.script_state.method_details {
        match method_detail.name.as_str() {
          "beforeMount" => {
            if !vue_imports.contains(&"onBeforeMount") {
              vue_imports.push("onBeforeMount");
            }
          }
          "mounted" => {
            if !vue_imports.contains(&"onMounted") {
              vue_imports.push("onMounted");
            }
          }
          "beforeUpdate" => {
            if !vue_imports.contains(&"onBeforeUpdate") {
              vue_imports.push("onBeforeUpdate");
            }
          }
          "updated" => {
            if !vue_imports.contains(&"onUpdated") {
              vue_imports.push("onUpdated");
            }
          }
          "beforeUnmount" => {
            if !vue_imports.contains(&"onBeforeUnmount") {
              vue_imports.push("onBeforeUnmount");
            }
          }
          "beforeDestroy" => {
            if !vue_imports.contains(&"onBeforeUnmount") {
              vue_imports.push("onBeforeUnmount");
            }
          }
          "destroyed" | "unmounted" => {
            if !vue_imports.contains(&"onUnmounted") {
              vue_imports.push("onUnmounted");
            }
          }
          "activated" => {
            if !vue_imports.contains(&"onActivated") {
              vue_imports.push("onActivated");
            }
          }
          "deactivated" => {
            if !vue_imports.contains(&"onDeactivated") {
              vue_imports.push("onDeactivated");
            }
          }
          _ => {}
        }
      }

      if !vue_imports.is_empty() {
        vue_imports.sort();
        vue_imports.dedup();
        result.add_imports(
          "vue",
          &vue_imports.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
        );
      }
    }

    // Collect and merge lifecycle methods by their Vue 3 equivalent
    let mut lifecycle_groups: std::collections::HashMap<&str, Vec<&crate::MethodDetail>> =
      std::collections::HashMap::new();

    for method_detail in &context.script_state.method_details {
      let vue3_hook = match method_detail.name.as_str() {
        "beforeCreate" | "created" => "setup", // These run directly in setup
        "beforeMount" => "onBeforeMount",
        "mounted" => "onMounted",
        "beforeUpdate" => "onBeforeUpdate",
        "updated" => "onUpdated",
        "beforeUnmount" => "onBeforeUnmount",
        "beforeDestroy" => "onBeforeUnmount", // Vue 2 beforeDestroy maps to Vue 3 onBeforeUnmount
        "destroyed" | "unmounted" => "onUnmounted", // Vue 2 destroyed maps to Vue 3 onUnmounted
        "activated" => "onActivated",
        "deactivated" => "onDeactivated",
        _ => continue,
      };

      lifecycle_groups
        .entry(vue3_hook)
        .or_default()
        .push(method_detail);
    }

    // Define the order for deterministic output
    let hook_order = [
      "setup",
      "onBeforeMount",
      "onMounted",
      "onBeforeUpdate",
      "onUpdated",
      "onBeforeUnmount",
      "onUnmounted",
      "onActivated",
      "onDeactivated",
    ];

    // Generate lifecycle code in deterministic order
    for vue3_hook in &hook_order {
      if let Some(methods) = lifecycle_groups.get(vue3_hook) {
        if vue3_hook == &"setup" {
          // beforeCreate and created run directly in setup
          for method_detail in methods {
            let body_transformer = TransformerOrchestrator::get_body_transformer();
            let transformed_body = body_transformer(&method_detail.body, context, config);

            for line in transformed_body.lines() {
              if !line.trim().is_empty() {
                result.lifecycle_hooks.push(line.to_string());
              }
            }
            result.lifecycle_hooks.push("".to_string()); // Add blank line
          }
        } else {
          // Other lifecycle hooks are wrapped in their Vue 3 equivalent
          result
            .lifecycle_hooks
            .push(format!("{}(() => {{", vue3_hook));

          for method_detail in methods {
            let body_transformer = TransformerOrchestrator::get_body_transformer();
            let transformed_body = body_transformer(&method_detail.body, context, config);

            for line in transformed_body.lines() {
              if !line.trim().is_empty() {
                result.lifecycle_hooks.push(format!("  {}", line));
              }
            }
          }

          result.lifecycle_hooks.push("});".to_string());
          result.lifecycle_hooks.push("".to_string()); // Add blank line
        }
      }
    }

    // Check if we need to import nextTick
    let has_next_tick = context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("this.$nextTick("))
      || context
        .script_state
        .computed_details
        .iter()
        .any(|computed| {
          if let Some(setter) = &computed.setter {
            setter.contains("this.$nextTick(")
          } else if let Some(getter) = &computed.getter {
            getter.contains("this.$nextTick(")
          } else {
            false
          }
        });

    if has_next_tick {
      result.add_import("vue", "nextTick");
    }

    // Handle template refs
    let template_refs = Self::extract_template_refs(context);
    if !template_refs.is_empty() {
      result.add_import("vue", "useTemplateRef");

      // Generate template ref declarations in reactive_state section (after regular refs)
      for ref_name in &template_refs {
        let var_name = Self::ref_name_to_variable(ref_name);
        result.reactive_state.push(format!(
          "const {} = useTemplateRef('{}');",
          var_name, ref_name
        ));
      }
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<super::BodyTransformFn>> {
    Some(Self::get_vue2_body_transform())
  }
}
