use super::Transformer;
use crate::{TemplateReplacement, TransformationContext, TransformationResult, TransformerConfig};

/// Transformer for rewriting imports and component names
///
/// This transformer handles:
/// 1. Rewriting import statements (e.g., bootstrap-vue -> bootstrap-vue-next)
/// 2. Rewriting component names in templates (e.g., BSidebar -> BOffcanvas)
/// 3. Rewriting component names in scripts (e.g., import rewrites)
/// 4. Adding additional imports for components that need them
/// 5. Rewriting directives (e.g., v-b-toggle -> vBToggle)
pub struct ImportRewriteTransformer;

impl Default for ImportRewriteTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportRewriteTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains imports that need rewriting
  fn has_rewritable_imports(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> bool {
    if let Some(import_rewrites) = &config.imports_rewrite {
      for import_info in &context.script_state.imports {
        if import_rewrites.contains_key(&import_info.source) {
          return true;
        }
      }
    }

    // Check for additional imports needed
    if let Some(additional_imports) = &config.additional_imports {
      // Check template for components that need additional imports
      for directive in &context.template_state.vue_directives {
        let tag = &directive.element_tag;
        if additional_imports.contains_key(tag) {
          return true;
        }
      }

      // Check for known component patterns in template
      if let Some(template_content) = &context.sfc_sections.template_content {
        for component_name in additional_imports.keys() {
          if template_content.contains(&format!("<{}", component_name.to_lowercase()))
            || template_content.contains(&format!("<{}", component_name))
          {
            return true;
          }
        }
      }
    }

    false
  }

  /// Generate new imports based on rewrite configuration
  fn generate_rewritten_imports(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<String> {
    let mut imports = Vec::new();

    if let Some(import_rewrites) = &config.imports_rewrite {
      for import_info in &context.script_state.imports {
        if let Some(rewrite_config) = import_rewrites.get(&import_info.source) {
          let mut import_items = Vec::new();

          // Collect component imports (with potential rewrites)
          for import_item in &import_info.imports {
            if !import_item.is_default && !import_item.is_namespace {
              let component_name = &import_item.name;

              // Check if this component needs to be rewritten
              let final_name = if let Some(component_rewrites) = &rewrite_config.component_rewrite {
                component_rewrites
                  .get(component_name)
                  .unwrap_or(component_name)
              } else {
                component_name
              };

              import_items.push(final_name.clone());
            }
          }

          // Add directive imports if they are used
          if let Some(directives) = &rewrite_config.directives {
            if let Some(template_content) = &context.sfc_sections.template_content {
              for (old_directive, new_directive) in directives {
                if template_content.contains(old_directive) {
                  import_items.push(new_directive.clone());
                }
              }
            }
          }

          if !import_items.is_empty() {
            import_items.sort(); // Keep consistent ordering
            let import_statement = format!(
              "import {{ {} }} from '{}';",
              import_items.join(", "),
              rewrite_config.name
            );
            imports.push(import_statement);
          }
        }
      }
    }

    // Add additional imports
    if let Some(additional_imports) = &config.additional_imports {
      if let Some(template_content) = &context.sfc_sections.template_content {
        for (component_name, import_config) in additional_imports {
          let should_import = template_content
            .contains(&format!("<{}", component_name.to_lowercase()))
            || template_content.contains(&format!("<{}", component_name));

          if should_import {
            if let Some(import_path) = &import_config.import_path {
              imports.push(import_path.clone());
            }
          }
        }
      }
    }

    imports
  }

  /// Generate template replacements for component names
  fn generate_template_replacements(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> Vec<TemplateReplacement> {
    let mut replacements = Vec::new();

    if let Some(import_rewrites) = &config.imports_rewrite {
      for import_info in &context.script_state.imports {
        if let Some(rewrite_config) = import_rewrites.get(&import_info.source) {
          if let Some(component_rewrites) = &rewrite_config.component_rewrite {
            for (old_component, new_component) in component_rewrites {
              // Replace both PascalCase and kebab-case versions
              replacements.push(TemplateReplacement {
                find: format!("<{}", old_component),
                replace: format!("<{}", new_component),
              });
              replacements.push(TemplateReplacement {
                find: format!("</{}>", old_component),
                replace: format!("</{}>", new_component),
              });

              // Convert to kebab-case and replace
              let old_kebab = to_kebab_case(old_component);
              let new_kebab = to_kebab_case(new_component);
              replacements.push(TemplateReplacement {
                find: format!("<{}", old_kebab),
                replace: format!("<{}", new_kebab),
              });
              replacements.push(TemplateReplacement {
                find: format!("</{}>", old_kebab),
                replace: format!("</{}>", new_kebab),
              });
            }
          }
        }
      }
    }

    // Handle additional imports with rewrite_to
    if let Some(additional_imports) = &config.additional_imports {
      for (component_name, import_config) in additional_imports {
        if let Some(rewrite_to) = &import_config.rewrite_to {
          let old_kebab = to_kebab_case(component_name);
          replacements.push(TemplateReplacement {
            find: format!("<{}", old_kebab),
            replace: format!("<{}", rewrite_to),
          });
          replacements.push(TemplateReplacement {
            find: format!("</{}>", old_kebab),
            replace: format!("</{}>", rewrite_to),
          });
        }
      }
    }

    replacements
  }
}

impl Transformer for ImportRewriteTransformer {
  fn name(&self) -> &'static str {
    "import_rewrite"
  }

  fn should_transform(&self, context: &TransformationContext, config: &TransformerConfig) -> bool {
    self.has_rewritable_imports(context, config)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    // Generate new imports (using special key for pre-formatted imports)
    let new_imports = self.generate_rewritten_imports(context, config);
    if !new_imports.is_empty() {
      result
        .imports_to_add
        .insert("__rewritten__".to_string(), new_imports);
    }

    // Generate template replacements
    let template_replacements = self.generate_template_replacements(context, config);
    result.template_replacements.extend(template_replacements);

    result
  }
}

/// Convert PascalCase to kebab-case
/// E.g., "BSidebar" -> "b-sidebar"
fn to_kebab_case(s: &str) -> String {
  let mut result = String::new();
  for (i, c) in s.chars().enumerate() {
    if i > 0 && c.is_uppercase() {
      result.push('-');
    }
    result.push(c.to_lowercase().next().unwrap());
  }
  result
}
