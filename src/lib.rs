use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use tree_sitter::{Node, Parser};

lazy_static! {
  static ref MUSTACHE_PATTERN: Regex = Regex::new(r"(?s)\{\{(.*?)\}\}").unwrap();
}

// Re-export transformers module
pub mod transformers;

#[derive(Debug, Clone, Default)]
pub struct RewriteOptions {
  pub mixins: Option<HashMap<String, MixinConfig>>,
  pub imports_rewrite: Option<HashMap<String, ImportRewrite>>,
  pub additional_imports: Option<HashMap<String, AdditionalImport>>,
  pub import_keeplist: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct MixinConfig {
  pub name: String,
  pub imports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportRewrite {
  pub name: String,
  pub component_rewrite: Option<HashMap<String, String>>,
  pub directives: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct AdditionalImport {
  pub import_path: Option<String>,
  pub rewrite_to: Option<String>,
}

pub fn rewrite_sfc(
  sfc: &str,
  options: Option<RewriteOptions>,
) -> Result<String, Box<dyn std::error::Error>> {
  // Parse the SFC sections
  let sections = parse_sfc_sections(sfc)?;

  // Initialize parsing states
  let mut script_state = ScriptParsingState::new();
  let mut template_state = TemplateParsingState::new();

  // Parse script section if present
  if let Some(script_content) = &sections.script_content {
    parse_script_section(script_content, &mut script_state)?;
  }

  // Parse template section if present
  if let Some(template_content) = &sections.template_content {
    parse_template_section(template_content, &mut template_state)?;
  }

  // Create transformation context
  let transformation_context = TransformationContext {
    script_state,
    template_state,
    sfc_sections: sections.clone(),
  };

  // Configure transformers - enable all by default for now
  let mut config = TransformerConfig {
    enable_i18n: true,
    enable_asset_transforms: true,
    ..Default::default()
  };

  // Apply options if provided
  if let Some(opts) = options {
    config.mixins = opts.mixins;
    config.imports_rewrite = opts.imports_rewrite;
    config.additional_imports = opts.additional_imports;
    config.import_keeplist = opts.import_keeplist;
  }

  // Apply transformations using the orchestrator
  let orchestrator = transformers::TransformerOrchestrator::new();
  let transformation_result = orchestrator.transform(&transformation_context, &config);

  // Build the final SFC
  let mut result_sfc = String::new();

  // Add template section
  if let Some(template_content) = &sections.template_content {
    let mut final_template = template_content.clone();

    // Apply template replacements
    for replacement in &transformation_result.template_replacements {
      final_template = final_template.replace(&replacement.find, &replacement.replace);
    }

    result_sfc.push_str("<template>\n");
    result_sfc.push_str(&final_template);
    result_sfc.push_str("\n</template>\n");
  }

  // Add script setup section
  result_sfc.push_str("<script setup>\n");

  // Add imports
  let formatted_imports = format_imports(&transformation_result.imports_to_add);
  for import in &formatted_imports {
    result_sfc.push_str(import);
    result_sfc.push('\n');
  }

  if !formatted_imports.is_empty() {
    result_sfc.push('\n');
  }

  // Add structured code sections in the correct order
  let mut sections_added = false;

  // 1. Setup code (composables, stores, router, etc.)
  if !transformation_result.setup.is_empty() {
    for line in &transformation_result.setup {
      // Rewrite ~/ to @/ in dynamic imports
      let rewritten_line = line.replace("'~/", "'@/").replace("\"~/", "\"@/");
      result_sfc.push_str(&rewritten_line);
      result_sfc.push('\n');
    }
    sections_added = true;
  }

  // 2. Reactive state (ref and reactive declarations)
  if !transformation_result.reactive_state.is_empty() {
    if sections_added {
      result_sfc.push('\n');
    }
    for line in &transformation_result.reactive_state {
      // Rewrite ~/ to @/ in dynamic imports
      let rewritten_line = line.replace("'~/", "'@/").replace("\"~/", "\"@/");
      result_sfc.push_str(&rewritten_line);
      result_sfc.push('\n');
    }
    sections_added = true;
  }

  // 3. Computed properties
  if !transformation_result.computed_properties.is_empty() {
    if sections_added {
      result_sfc.push('\n');
    }
    for line in &transformation_result.computed_properties {
      // Rewrite ~/ to @/ in dynamic imports
      let rewritten_line = line.replace("'~/", "'@/").replace("\"~/", "\"@/");
      result_sfc.push_str(&rewritten_line);
      result_sfc.push('\n');
    }
    sections_added = true;
  }

  // 4. Watchers
  if !transformation_result.watchers.is_empty() {
    if sections_added {
      result_sfc.push('\n');
    }
    for line in &transformation_result.watchers {
      result_sfc.push_str(line);
      result_sfc.push('\n');
    }
    sections_added = true;
  }

  // 5. Methods
  if !transformation_result.methods.is_empty() {
    if sections_added {
      result_sfc.push('\n');
    }
    for line in &transformation_result.methods {
      // Rewrite ~/ to @/ in dynamic imports
      let rewritten_line = line.replace("'~/", "'@/").replace("\"~/", "\"@/");
      result_sfc.push_str(&rewritten_line);
      result_sfc.push('\n');
    }
    sections_added = true;
  }

  // 6. Lifecycle hooks
  if !transformation_result.lifecycle_hooks.is_empty() {
    if sections_added {
      result_sfc.push('\n');
    }
    for line in &transformation_result.lifecycle_hooks {
      // Rewrite ~/ to @/ in dynamic imports
      let rewritten_line = line.replace("'~/", "'@/").replace("\"~/", "\"@/");
      result_sfc.push_str(&rewritten_line);
      result_sfc.push('\n');
    }
  }

  result_sfc.push_str("</script>");

  // Add additional script blocks (with path rewriting)
  for script_block in &transformation_result.additional_scripts {
    result_sfc.push('\n');
    // Rewrite ~/ to @/ in dynamic imports
    let rewritten_block = script_block.replace("'~/", "'@/").replace("\"~/", "\"@/");
    result_sfc.push_str(&rewritten_block);
  }

  // Add style section if present
  if let Some(style_content) = &sections.style_content {
    result_sfc.push_str("\n<style");
    if let Some(attributes) = &sections.style_attributes {
      result_sfc.push(' ');
      result_sfc.push_str(attributes);
    }
    result_sfc.push_str(">\n");
    result_sfc.push_str(style_content);
    result_sfc.push_str("\n</style>");
  }

  Ok(result_sfc)
}

/// Format the imports HashMap into a sorted list of import statements
fn format_imports(imports_map: &HashMap<String, Vec<String>>) -> Vec<String> {
  let mut result = Vec::new();

  // Convert to vec for sorting
  let mut imports: Vec<(&String, &Vec<String>)> = imports_map.iter().collect();

  // Sort imports: Vue imports first, then node_modules, then relative imports
  imports.sort_by(|(path_a, _), (path_b, _)| {
    let a_is_vue = *path_a == "vue";
    let b_is_vue = *path_b == "vue";
    let a_is_relative =
      path_a.starts_with("@/") || path_a.starts_with("./") || path_a.starts_with("../");
    let b_is_relative =
      path_b.starts_with("@/") || path_b.starts_with("./") || path_b.starts_with("../");

    match (a_is_vue, b_is_vue, a_is_relative, b_is_relative) {
      (true, false, _, _) => std::cmp::Ordering::Less, // Vue imports first
      (false, true, _, _) => std::cmp::Ordering::Greater, // Vue imports first
      (_, _, false, true) => std::cmp::Ordering::Less, // node_modules before relative
      (_, _, true, false) => std::cmp::Ordering::Greater, // relative after node_modules
      (_, _, true, true) => {
        // Within relative imports, prefer stores before composables
        let a_is_store = path_a.starts_with("@/stores/");
        let b_is_store = path_b.starts_with("@/stores/");
        let a_is_composable = path_a.starts_with("@/composables/");
        let b_is_composable = path_b.starts_with("@/composables/");

        match (a_is_store, b_is_store, a_is_composable, b_is_composable) {
          (true, false, _, true) => std::cmp::Ordering::Less, // stores before composables
          (false, true, true, _) => std::cmp::Ordering::Greater, // stores before composables
          _ => path_a.cmp(path_b),                            // alphabetical otherwise
        }
      }
      _ => path_a.cmp(path_b), // alphabetical within the same group
    }
  });

  for (path, import_items) in imports {
    if import_items.is_empty() {
      continue;
    }

    if path.starts_with("__") {
      // Special cases for pre-formatted imports (e.g., from import_rewrite transformer)
      result.extend(import_items.clone());
    } else {
      // Deduplicate and sort import items
      let mut unique_items = import_items.clone();
      unique_items.sort();
      unique_items.dedup();

      // Format the import statement
      result.push(format!(
        "import {{ {} }} from '{}';",
        unique_items.join(", "),
        path
      ));
    }
  }

  result
}

/// Rewrite import paths to use standard aliases
fn rewrite_import_path(path: &str) -> String {
  // Rewrite ~/ to @/ (common Nuxt.js alias conversion)
  if let Some(stripped) = path.strip_prefix("~/") {
    format!("@/{}", stripped)
  } else {
    path.to_string()
  }
}

// Parser structures
#[derive(Debug, Clone)]
pub struct ParsedSFC {
  pub template: Option<ParsedTemplate>,
  pub script: Option<ParsedScript>,
  pub style: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedTemplate {
  pub content: String,
  pub directives: Vec<VueDirective>,
  pub mustache_expressions: Vec<MustacheExpression>,
}

#[derive(Debug, Clone)]
pub struct VueDirective {
  pub name: String,
  pub value: String,
  pub element: String,
  pub variables_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MustacheExpression {
  pub content: String,
  pub variables_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedScript {
  pub name: Option<String>,
  pub props: Option<HashMap<String, PropDefinition>>,
  pub data: Option<HashMap<String, String>>,
  pub computed: Option<HashMap<String, ComputedProperty>>,
  pub methods: Option<HashMap<String, MethodDefinition>>,
  pub watch: Option<HashMap<String, WatcherDefinition>>,
  pub lifecycle_hooks: Option<HashMap<String, LifecycleHook>>,
  pub other_options: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PropDefinition {
  pub prop_type: String,
  pub required: bool,
  pub default: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComputedProperty {
  pub getter: Option<String>,
  pub setter: Option<String>,
  pub variables_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MethodDefinition {
  pub body: String,
  pub is_async: bool,
  pub variables_used: Vec<String>,
  pub methods_called: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WatcherDefinition {
  pub handler: String,
  pub variables_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LifecycleHook {
  pub body: String,
  pub variables_used: Vec<String>,
  pub methods_called: Vec<String>,
}

/// Represents the parsed sections of a Vue Single File Component (SFC).
#[derive(Debug, Clone, PartialEq)]
pub struct SfcSections {
  /// Content inside the `<template>` tag
  pub template_content: Option<String>,
  /// Content inside the `<script>` tag
  pub script_content: Option<String>,
  /// Content inside the `<style>` tag
  pub style_content: Option<String>,
  /// Attributes of the `<style>` tag (e.g., "scoped", "lang='scss'")
  pub style_attributes: Option<String>,
}

/// Parses a Vue Single File Component (SFC) string into its main sections.
///
/// This function extracts the content from root-level `<template>`, `<script>`, and `<style>` tags
/// while preserving any nested HTML tags within those sections. Only the first occurrence of each
/// section type is captured.
///
/// # Arguments
///
/// * `sfc_content` - The SFC string containing Vue component markup
///
/// # Returns
///
/// Returns a `Result<SfcSections, Box<dyn std::error::Error>>` containing the parsed sections
/// or an error if parsing fails.
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::parse_sfc_sections;
///
/// let sfc = r#"<template>
///   <div class="container">
///     <h1>{{ title }}</h1>
///     <p>Nested <span>content</span> preserved</p>
///   </div>
/// </template>
/// <script>
/// export default {
///   name: 'MyComponent',
///   data() {
///     return {
///       title: 'Hello World'
///     };
///   }
/// }
/// </script>
/// <style scoped>
/// .container {
///   margin: 0 auto;
/// }
/// </style>"#;
///
/// let sections = parse_sfc_sections(sfc).unwrap();
///
/// assert!(sections.template_content.is_some());
/// assert!(sections.script_content.is_some());
/// assert!(sections.style_content.is_some());
///
/// // Template content preserves nested HTML
/// let template = sections.template_content.unwrap();
/// assert!(template.contains("<div class=\"container\">"));
/// assert!(template.contains("<span>content</span>"));
///
/// // Script content is extracted properly
/// let script = sections.script_content.unwrap();
/// assert!(script.contains("export default"));
/// assert!(script.contains("title: 'Hello World'"));
///
/// // Style content is extracted properly
/// let style = sections.style_content.unwrap();
/// assert!(style.contains(".container"));
/// assert!(style.contains("margin: 0 auto"));
/// ```
///
/// ```
/// use vue_options_to_composition::parse_sfc_sections;
///
/// // Example with missing sections
/// let minimal_sfc = r#"<template><p>Just a template</p></template>"#;
/// let sections = parse_sfc_sections(minimal_sfc).unwrap();
///
/// assert!(sections.template_content.is_some());
/// assert!(sections.script_content.is_none());
/// assert!(sections.style_content.is_none());
/// ```
///
/// ```
/// use vue_options_to_composition::parse_sfc_sections;
///
/// // Example with nested template tags (only root level extracted)
/// let nested_sfc = r#"
/// <template>
///   <div>
///     <template v-if="showContent">
///       <p>Conditional content</p>
///     </template>
///   </div>
/// </template>
/// <script>
/// const template = '<template>Not extracted</template>';
/// export default { name: 'Test' };
/// </script>"#;
///
/// let sections = parse_sfc_sections(nested_sfc).unwrap();
/// let template = sections.template_content.unwrap();
///
/// // Nested template tags are preserved as content
/// assert!(template.contains("<template v-if=\"showContent\">"));
/// assert!(template.contains("Conditional content"));
///
/// let script = sections.script_content.unwrap();
/// // Template string in script is preserved as-is
/// assert!(script.contains("'<template>Not extracted</template>'"));
/// ```
pub fn parse_sfc_sections(sfc_content: &str) -> Result<SfcSections, Box<dyn std::error::Error>> {
  let mut template_content: Option<String> = None;
  let mut script_content: Option<String> = None;
  let mut style_content: Option<String> = None;
  let mut style_attributes: Option<String> = None;

  // Extract content using string parsing since lol_html text handlers are complex for this use case

  // Extract template content
  if let Some(start) = sfc_content.find("<template") {
    if let Some(content_start) = sfc_content[start..].find('>') {
      let content_start = start + content_start + 1;
      if let Some(end) = find_closing_tag(sfc_content, content_start, "template") {
        let content = sfc_content[content_start..end].trim();
        if !content.is_empty() {
          template_content = Some(content.to_string());
        }
      }
    }
  }

  // Extract script content
  if let Some(start) = sfc_content.find("<script") {
    if let Some(content_start) = sfc_content[start..].find('>') {
      let content_start = start + content_start + 1;
      if let Some(end) = find_closing_tag(sfc_content, content_start, "script") {
        let content = sfc_content[content_start..end].trim();
        if !content.is_empty() {
          script_content = Some(content.to_string());
        }
      }
    }
  }

  // Extract style content
  if let Some(start) = sfc_content.find("<style") {
    if let Some(tag_end) = sfc_content[start..].find('>') {
      let tag_end_absolute = start + tag_end;
      let content_start = tag_end_absolute + 1;

      // Extract style tag attributes (everything between <style and >)
      let tag_content = &sfc_content[start + 6..tag_end_absolute]; // Skip "<style"
      let attributes = tag_content.trim();
      if !attributes.is_empty() {
        style_attributes = Some(attributes.to_string());
      }

      // Extract style content
      if let Some(end) = find_closing_tag(sfc_content, content_start, "style") {
        let content = sfc_content[content_start..end].trim();
        if !content.is_empty() {
          style_content = Some(content.to_string());
        }
      }
    }
  }

  Ok(SfcSections {
    template_content,
    script_content,
    style_content,
    style_attributes,
  })
}

/// Helper function to find the closing tag while respecting nesting
fn find_closing_tag(content: &str, start: usize, tag_name: &str) -> Option<usize> {
  let search_content = &content[start..];
  let opening_tag = format!("<{}", tag_name);
  let closing_tag = format!("</{}>", tag_name);

  let mut depth = 1;
  let mut pos = 0;

  while depth > 0 && pos < search_content.len() {
    if let Some(open_pos) = search_content[pos..].find(&opening_tag) {
      if let Some(close_pos) = search_content[pos..].find(&closing_tag) {
        let adjusted_open_pos = pos + open_pos;
        let adjusted_close_pos = pos + close_pos;

        if adjusted_open_pos < adjusted_close_pos {
          // Found an opening tag first
          depth += 1;
          pos = adjusted_open_pos + opening_tag.len();
        } else {
          // Found a closing tag first
          depth -= 1;
          if depth == 0 {
            return Some(start + adjusted_close_pos);
          }
          pos = adjusted_close_pos + closing_tag.len();
        }
      } else {
        // No more closing tags, malformed
        return None;
      }
    } else if let Some(close_pos) = search_content[pos..].find(&closing_tag) {
      // Found a closing tag with no more opening tags
      depth -= 1;
      if depth == 0 {
        return Some(start + pos + close_pos);
      }
      pos = pos + close_pos + closing_tag.len();
    } else {
      // No more tags found
      return None;
    }
  }

  None
}

/// Represents the state that accumulates parsing results across different script elements.
#[derive(Debug, Clone)]
pub struct ScriptParsingState {
  pub identifiers: Vec<String>,
  pub function_calls: Vec<String>,
  pub function_call_details: Vec<FunctionCallDetail>,
  pub methods: Vec<String>,
  pub method_details: Vec<MethodDetail>,
  pub computed_properties: Vec<String>,
  pub computed_details: Vec<ComputedDetail>,
  pub imports: Vec<ImportInfo>,
  pub setup_content: Option<String>,
  pub props: Vec<PropInfo>,
  pub data_properties: Vec<DataPropertyInfo>,
  pub head_method: Option<MethodDetail>,
  pub fetch_method: Option<MethodDetail>,
  pub watchers: Vec<WatcherDetail>,
  pub nuxt_i18n: Option<String>, // Raw nuxtI18n object content
  pub async_data_method: Option<String>,
}

/// Information about a method definition with its body.
#[derive(Debug, Clone)]
pub struct MethodDetail {
  pub name: String,
  pub parameters: Vec<String>, // Method parameter names
  pub body: String,
  pub is_async: bool,
}

/// Information about a watcher definition.
#[derive(Debug, Clone)]
pub struct WatcherDetail {
  pub watched_property: String,
  pub handler_body: String,
  pub is_async: bool,
  pub param_names: (String, String), // (newVal, oldVal) parameter names
}

/// Information about a computed property definition with its getter/setter.
#[derive(Debug, Clone)]
pub struct ComputedDetail {
  pub name: String,
  pub getter: Option<String>,
  pub setter: Option<String>,
  pub setter_parameter: Option<String>, // Parameter name for setter (e.g., "value", "v")
  pub is_simple_function: bool, // true for computed: () => expr, false for { get, set }
}

/// Information about a prop definition.
#[derive(Debug, Clone)]
pub struct PropInfo {
  pub name: String,
  pub prop_type: Option<String>,
  pub required: Option<bool>,
  pub default_value: Option<String>,
  pub validator: Option<String>,
}

/// Information about a data property.
#[derive(Debug, Clone)]
pub struct DataPropertyInfo {
  pub name: String,
  pub value: Option<String>,
}

/// Information about an import statement found in the script.
#[derive(Debug, Clone)]
pub struct ImportInfo {
  pub source: String,
  pub imports: Vec<ImportItem>,
}

/// Individual import item (default, named, namespace).
#[derive(Debug, Clone)]
pub struct ImportItem {
  pub name: String,
  pub alias: Option<String>,
  pub is_default: bool,
  pub is_namespace: bool,
}

/// Represents the state that accumulates parsing results from template parsing.
#[derive(Debug, Clone)]
pub struct TemplateParsingState {
  pub identifiers: Vec<String>,
  pub function_calls: Vec<String>,
  pub function_call_details: Vec<FunctionCallDetail>,
  pub vue_directives: Vec<VueDirectiveInfo>,
  pub mustache_expressions: Vec<MustacheExpressionInfo>,
}

/// Information about a Vue directive found in the template.
#[derive(Debug, Clone)]
pub struct VueDirectiveInfo {
  pub name: String,
  pub value: String,
  pub element_tag: String,
}

/// Information about a mustache expression found in the template.
#[derive(Debug, Clone)]
pub struct MustacheExpressionInfo {
  pub content: String,
}

impl Default for ScriptParsingState {
  fn default() -> Self {
    Self::new()
  }
}

impl ScriptParsingState {
  pub fn new() -> Self {
    Self {
      identifiers: Vec::new(),
      function_calls: Vec::new(),
      function_call_details: Vec::new(),
      methods: Vec::new(),
      method_details: Vec::new(),
      computed_properties: Vec::new(),
      computed_details: Vec::new(),
      imports: Vec::new(),
      setup_content: None,
      props: Vec::new(),
      data_properties: Vec::new(),
      head_method: None,
      fetch_method: None,
      watchers: Vec::new(),
      nuxt_i18n: None,
      async_data_method: None,
    }
  }
}

impl Default for TemplateParsingState {
  fn default() -> Self {
    Self::new()
  }
}

impl TemplateParsingState {
  pub fn new() -> Self {
    Self {
      identifiers: Vec::new(),
      function_calls: Vec::new(),
      function_call_details: Vec::new(),
      vue_directives: Vec::new(),
      mustache_expressions: Vec::new(),
    }
  }
}

/// Parses a Vue script section using tree-sitter to extract identifiers, function calls, methods, and computed properties.
///
/// This function analyzes the JavaScript code within a Vue component's script section and identifies:
/// - **Identifiers**: Variable names and property references
/// - **Function calls**: Functions being invoked
/// - **Methods**: Functions defined in the component's methods object
/// - **Computed properties**: Properties defined in the component's computed object
///
/// The parsing is performed using tree-sitter for accurate JavaScript AST analysis, ensuring proper
/// handling of complex nested structures and edge cases.
///
/// # Arguments
///
/// * `script_content` - The JavaScript content from within the `<script>` tag
/// * `state` - Mutable reference to the parsing state that accumulates results
///
/// # Returns
///
/// Returns `Result<(), Box<dyn std::error::Error>>` indicating success or parsing failure.
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::{parse_script_section, ScriptParsingState};
///
/// let script = r#"
/// export default {
///   name: 'TestComponent',
///   data() {
///     return {
///       count: 0,
///       items: []
///     };
///   },
///   computed: {
///     itemCount() {
///       return this.items.length;
///     },
///     formattedCount: {
///       get() {
///         return this.count.toString();
///       },
///       set(value) {
///         this.count = parseInt(value);
///       }
///     }
///   },
///   methods: {
///     increment() {
///       this.count++;
///     },
///     fetchData() {
///       return this.$axios.get('/api/data');
///     }
///   }
/// }
/// "#;
///
/// let mut state = ScriptParsingState::new();
/// parse_script_section(script, &mut state).unwrap();
///
/// // Check that methods were identified
/// assert!(state.methods.contains(&"increment".to_string()));
/// assert!(state.methods.contains(&"fetchData".to_string()));
///
/// // Check that computed properties were identified
/// assert!(state.computed_properties.contains(&"itemCount".to_string()));
/// assert!(state.computed_properties.contains(&"formattedCount".to_string()));
///
/// // Check that function calls were identified
/// assert!(state.function_calls.iter().any(|call| call.contains("get")));
///
/// // Check that identifiers were identified
/// assert!(state.identifiers.iter().any(|id| id == "count" || id == "items"));
/// ```
///
/// ```
/// use vue_options_to_composition::{parse_script_section, ScriptParsingState};
///
/// // Example with complex nested structure
/// let complex_script = r#"
/// import { mapActions, mapGetters } from 'vuex';
///
/// export default {
///   methods: {
///     async handleSubmit() {
///       const result = await this.validateForm();
///       if (result.isValid) {
///         this.submitData(result.data);
///       }
///     },
///     ...mapActions(['submitData', 'validateForm'])
///   },
///   computed: {
///     ...mapGetters(['user', 'permissions']),
///     canSubmit() {
///       return this.user && this.permissions.write;
///     }
///   }
/// }
/// "#;
///
/// let mut state = ScriptParsingState::new();
/// parse_script_section(complex_script, &mut state).unwrap();
///
/// assert!(state.methods.contains(&"handleSubmit".to_string()));
/// assert!(state.computed_properties.contains(&"canSubmit".to_string()));
/// ```
pub fn parse_script_section(
  script_content: &str,
  state: &mut ScriptParsingState,
) -> Result<(), Box<dyn std::error::Error>> {
  // First extract imports and setup content using string parsing
  extract_imports_and_setup(script_content, state)?;

  // Then use tree-sitter for Vue component structure
  let language = tree_sitter_javascript::LANGUAGE.into();
  let mut parser = Parser::new();
  parser.set_language(&language)?;

  let tree = parser
    .parse(script_content, None)
    .ok_or("Failed to parse script content")?;
  let root_node = tree.root_node();

  // Walk the AST to find Vue component structure
  find_vue_component_sections(&root_node, script_content, state);

  Ok(())
}

/// Extracts imports and setup content using string parsing
fn extract_imports_and_setup(
  script_content: &str,
  state: &mut ScriptParsingState,
) -> Result<(), Box<dyn std::error::Error>> {
  let lines: Vec<&str> = script_content.lines().collect();
  let mut setup_lines = Vec::new();
  let mut current_import = String::new();
  let mut in_multiline_import = false;

  for line in &lines {
    let trimmed = line.trim();

    if trimmed.starts_with("import ") {
      if trimmed.contains(" from ") {
        // Single-line import
        if let Some(import_info) = parse_import_line(trimmed) {
          state.imports.push(import_info);
        }
      } else {
        // Start of multi-line import
        current_import = trimmed.to_string();
        in_multiline_import = true;
      }
    } else if in_multiline_import {
      // Continue building the multi-line import
      current_import.push(' ');
      current_import.push_str(trimmed);

      if trimmed.contains(" from ") {
        // End of multi-line import
        if let Some(import_info) = parse_import_line(&current_import) {
          state.imports.push(import_info);
        }
        in_multiline_import = false;
        current_import.clear();
      }
    } else if trimmed.starts_with("export default") {
      break;
    } else if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
      // This is setup content (content after imports but before export default)
      setup_lines.push(*line);
    }
  }

  if !setup_lines.is_empty() {
    state.setup_content = Some(setup_lines.join("\n"));
  }

  Ok(())
}

/// Parses a single import line to extract import information
fn parse_import_line(line: &str) -> Option<ImportInfo> {
  // Basic regex-based parsing for common import patterns
  if let Some(from_pos) = line.find(" from ") {
    let import_part = line[..from_pos].trim();
    let source_part = line[from_pos + 6..].trim();

    // Extract source (remove quotes and semicolon)
    let mut source = source_part.trim_matches(';').trim();
    // Remove quotes - handle both single and double quotes
    if (source.starts_with('\'') && source.ends_with('\''))
      || (source.starts_with('"') && source.ends_with('"'))
    {
      source = &source[1..source.len() - 1];
    }

    let mut imports = Vec::new();

    // Remove "import " from the beginning
    let import_content = import_part
      .strip_prefix("import")
      .unwrap_or(import_part)
      .trim();

    if import_content.starts_with('{') && import_content.ends_with('}') {
      // Named imports: import { a, b, c } from 'module'
      let named_imports = &import_content[1..import_content.len() - 1];
      for item in named_imports.split(',') {
        let item = item.trim();
        if !item.is_empty() {
          if let Some(as_pos) = item.find(" as ") {
            let name = item[..as_pos].trim();
            let alias = item[as_pos + 4..].trim();
            imports.push(ImportItem {
              name: name.to_string(),
              alias: Some(alias.to_string()),
              is_default: false,
              is_namespace: false,
            });
          } else {
            imports.push(ImportItem {
              name: item.to_string(),
              alias: None,
              is_default: false,
              is_namespace: false,
            });
          }
        }
      }
    } else if import_content.starts_with("* as ") {
      // Namespace import: import * as name from 'module'
      let name = import_content.strip_prefix("* as ").unwrap_or("").trim();
      imports.push(ImportItem {
        name: name.to_string(),
        alias: None,
        is_default: false,
        is_namespace: true,
      });
    } else {
      // Default import: import name from 'module'
      imports.push(ImportItem {
        name: import_content.to_string(),
        alias: None,
        is_default: true,
        is_namespace: false,
      });
    }

    return Some(ImportInfo {
      source: rewrite_import_path(source),
      imports,
    });
  }

  None
}

/// Recursively walks the AST to find Vue component sections (methods, computed, etc.)
fn find_vue_component_sections(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Look for variable declarations (for async components)
  if node.kind() == "variable_declaration" {
    parse_variable_declarations(node, source, state);
  }

  // Look for export default object
  if node.kind() == "export_statement" {
    // Look for the value field which contains the exported object
    if let Some(value_node) = node.child_by_field_name("value") {
      if value_node.kind() == "object" {
        parse_vue_component_object(&value_node, source, state);
        return;
      }
    }

    // Fallback: iterate through children to find object
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "object" {
          parse_vue_component_object(&child, source, state);
          return;
        }
      }
    }
  }

  // Look for direct object expressions (for cases like just { methods: {...} })
  if node.kind() == "object" {
    parse_vue_component_object(node, source, state);
    return;
  }

  // Continue searching in children
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      find_vue_component_sections(&child, source, state);
    }
  }
}

/// Parses variable declarations to detect async components
fn parse_variable_declarations(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Look for variable_declarator nodes
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "variable_declarator" {
        parse_async_component_declarator(&child, source, state);
      }
    }
  }
}

/// Parses a single variable declarator to check if it's an async component
fn parse_async_component_declarator(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Get the variable name
  if let Some(name_node) = node.child_by_field_name("name") {
    let _variable_name = get_node_text(&name_node, source);
    
    // Get the value (the right-hand side of assignment)
    if let Some(value_node) = node.child_by_field_name("value") {
      // Check if it's an arrow function that imports
      if is_async_component_declaration(&value_node, source) {
        // This is an async component declaration - store it in setup_content
        // The transformer will handle the conversion using the updated regex
        let _declaration_text = get_node_text(node, source);
        if let Some(ref mut _setup_content) = state.setup_content {
          // Already have setup content, this should be part of it
          // The regex-based approach in the transformer will handle the transformation
        } else {
          // This shouldn't happen since extract_imports_and_setup runs first,
          // but just in case, we don't need to do anything as the regex handles it
        }
      }
    }
  }
}

/// Check if a value node represents an async component declaration
fn is_async_component_declaration(node: &Node, source: &str) -> bool {
  if node.kind() == "arrow_function" {
    // Check if the body contains an import() call
    if let Some(body_node) = node.child_by_field_name("body") {
      return contains_dynamic_import(&body_node, source);
    }
  }
  false
}

/// Recursively check if a node contains a dynamic import() call
fn contains_dynamic_import(node: &Node, source: &str) -> bool {
  if node.kind() == "call_expression" {
    if let Some(function_node) = node.child_by_field_name("function") {
      let function_text = get_node_text(&function_node, source);
      if function_text == "import" {
        return true;
      }
    }
  }
  
  // Recursively check children
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if contains_dynamic_import(&child, source) {
        return true;
      }
    }
  }
  
  false
}

/// Parses the main Vue component object to extract methods, computed properties, etc.
fn parse_vue_component_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let key_text = get_node_text(&key_node, source);
          let key = key_text.trim_matches('"').trim_matches('\'');
          match key {
            "methods" => {
              parse_methods_object(&value_node, source, state);
            }
            "computed" => {
              parse_computed_object(&value_node, source, state);
            }
            "props" => {
              parse_props_object(&value_node, source, state);
            }
            "data" => {
              parse_data_function(&value_node, source, state);
            }
            "head" => {
              parse_head_method(&value_node, source, state);
            }
            "watch" => {
              // Parse watchers object specially
              parse_watchers_object(&value_node, source, state);
            }
            "nuxtI18n" => {
              // Extract the nuxtI18n object content
              let content = get_node_text(&value_node, source);
              state.nuxt_i18n = Some(content);
            }
            "asyncData" => {
              // Extract the asyncData method, should be kept as-is, because it is isolated
              let content = get_node_text(&value_node, source);
              state.async_data_method = Some(content);
            }
            "beforeCreate" | "created" | "beforeMount" | "mounted" | "beforeUpdate" | "updated"
            | "beforeDestroy" | "destroyed" | "beforeUnmount" | "unmounted" | "activated"
            | "deactivated" | "fetch" => {
              // Parse these sections for identifiers and function calls
              parse_general_node(&value_node, source, state);

              // Also create method details for lifecycle methods (needed for transformers)
              let is_async = check_if_async(&value_node, source);
              let body = extract_method_body(&value_node, source);

              state.method_details.push(MethodDetail {
                name: key.to_string(),
                parameters: Vec::new(), // TODO: Parse parameters
                body,
                is_async,
              });
            }
            _ => {
              // Parse any other properties for identifiers and function calls
              parse_general_node(&value_node, source, state);
            }
          }
        }
      } else if child.kind() == "method_definition" {
        // Handle method definitions like data() { ... } and head() { ... }
        if let Some(name_node) = child.child_by_field_name("name") {
          let method_text = get_node_text(&name_node, source);
          let method_name = method_text.trim_matches('"').trim_matches('\'');

          match method_name {
            "data" => {
              parse_data_function(&child, source, state);
            }
            "head" => {
              parse_head_method(&child, source, state);
            }
            "fetch" => {
              parse_fetch_method(&child, source, state);
            }
            "asyncData" => {
              // Extract the asyncData method, should be kept as-is, because it is isolated
              let content = get_node_text(&child, source);
              state.async_data_method = Some(content);
            }
            _ => {
              // Handle lifecycle methods and other function definitions
              parse_general_node(&child, source, state);

              // Create method details for lifecycle methods
              if matches!(
                method_name,
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
                let is_async = check_if_async(&child, source);
                let body = extract_method_body(&child, source);

                state.method_details.push(MethodDetail {
                  name: method_name.to_string(),
                  parameters: Vec::new(), // TODO: Parse parameters
                  body,
                  is_async,
                });
              }
            }
          }
        }
      }
    }
  }
}

/// Parses the methods object to extract method names and their contents
fn parse_methods_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        // Handle method: function() { ... } syntax
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let method_text = get_node_text(&key_node, source);
          let method_name = method_text.trim_matches('"').trim_matches('\'');

          // Add to methods list for backward compatibility
          state.methods.push(method_name.to_string());

          // Extract method details
          let is_async = check_if_async(&value_node, source);
          let body = extract_method_body(&value_node, source);
          let parameters = extract_method_parameters(&value_node, source);

          state.method_details.push(MethodDetail {
            name: method_name.to_string(),
            parameters,
            body,
            is_async,
          });
        }
      } else if child.kind() == "method_definition" {
        // Handle shorthand method syntax: methodName() { ... }
        let mut body_index = 2;
        if let Some(name_node) = child.child_by_field_name("name") {
          let method_text = get_node_text(&name_node, source);
          let method_name = method_text.trim_matches('"').trim_matches('\'');

          // Add to methods list for backward compatibility
          state.methods.push(method_name.to_string());

          // Extract method details
          let is_async = check_if_async(&child, source);
          let body = extract_method_body(&child, source);
          let parameters = extract_method_parameters(&child, source);

          if is_async {
            body_index = 3; // Async methods have an extra parameter
          }

          state.method_details.push(MethodDetail {
            name: method_name.to_string(),
            parameters,
            body,
            is_async,
          });
        }

        // Parse the method body for identifiers and function calls
        if let Some(value_node) = child.child(body_index) {
          parse_general_node(&value_node, source, state);
        }
      } else if child.kind() == "spread_element" {
        // Handle ...mapActions, ...mapMutations etc.
        parse_general_node(&child, source, state);
      }
    }
  }
}

/// Parses the computed object to extract computed property names and their contents
fn parse_computed_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let computed_text = get_node_text(&key_node, source);
          let computed_name = computed_text.trim_matches('"').trim_matches('\'');

          // Add to computed properties list for backward compatibility
          state.computed_properties.push(computed_name.to_string());

          // Parse computed details
          let mut computed_detail = ComputedDetail {
            name: computed_name.to_string(),
            getter: None,
            setter: None,
            setter_parameter: None,
            is_simple_function: false,
          };

          // Check if it's a simple function: computed: () => expr
          if value_node.kind() == "arrow_function" || value_node.kind() == "function" {
            computed_detail.is_simple_function = true;
            computed_detail.getter = Some(extract_method_body(&value_node, source));
          }
          // Check if it's an object with get/set: computed: { get() {...}, set(v) {...} }
          else if value_node.kind() == "object" {
            parse_computed_getter_setter(&value_node, source, &mut computed_detail);
          }

          state.computed_details.push(computed_detail);
        }
      } else if child.kind() == "method_definition" {
        // Handle shorthand computed syntax: computedName() { ... }
        if let Some(name_node) = child.child_by_field_name("name") {
          let computed_text = get_node_text(&name_node, source);
          let computed_name = computed_text.trim_matches('"').trim_matches('\'');

          state.computed_properties.push(computed_name.to_string());

          let computed_detail = ComputedDetail {
            name: computed_name.to_string(),
            getter: Some(extract_method_body(&child, source)),
            setter: None,
            setter_parameter: None,
            is_simple_function: true,
          };

          state.computed_details.push(computed_detail);
        }

        // Parse the computed property body for identifiers and function calls
        parse_general_node(&child, source, state);
      } else if child.kind() == "spread_element" {
        // Handle ...mapGetters, ...mapState etc.
        parse_general_node(&child, source, state);
      }
    }
  }
}

/// Parse getter/setter from a computed property object
fn parse_computed_getter_setter(
  object_node: &Node,
  source: &str,
  computed_detail: &mut ComputedDetail,
) {
  for i in 0..object_node.child_count() {
    if let Some(child) = object_node.child(i) {
      if child.kind() == "method_definition" {
        if let Some(name_node) = child.child_by_field_name("name") {
          let method_name = get_node_text(&name_node, source);
          let body = extract_method_body(&child, source);

          match method_name.as_str() {
            "get" => computed_detail.getter = Some(body),
            "set" => {
              computed_detail.setter = Some(body);
              // Extract setter parameter name
              let params = extract_method_parameters(&child, source);
              if !params.is_empty() {
                computed_detail.setter_parameter = Some(params[0].clone());
              }
            },
            _ => {}
          }
        }
      } else if child.kind() == "pair" {
        // Handle get: function() {...} syntax
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let method_text = get_node_text(&key_node, source);
          let method_name = method_text.trim_matches('"').trim_matches('\'');
          let body = extract_method_body(&value_node, source);

          match method_name {
            "get" => computed_detail.getter = Some(body),
            "set" => {
              computed_detail.setter = Some(body);
              // Extract setter parameter name
              let params = extract_method_parameters(&value_node, source);
              if !params.is_empty() {
                computed_detail.setter_parameter = Some(params[0].clone());
              }
            },
            _ => {}
          }
        }
      }
    }
  }
}

/// Parses the props object to extract prop definitions
fn parse_props_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        if let Some(key_node) = child.child(0) {
          let prop_text = get_node_text(&key_node, source);
          let prop_name = prop_text.trim_matches('"').trim_matches('\'');

          let mut prop_info = PropInfo {
            name: prop_name.to_string(),
            prop_type: None,
            required: None,
            default_value: None,
            validator: None,
          };

          // Parse prop definition (could be object with type, required, default, etc.)
          if let Some(value_node) = child.child(2) {
            if value_node.kind() == "object" {
              // Complex prop definition: { type: String, required: true, default: 'value' }
              parse_prop_definition(&value_node, source, &mut prop_info);
            } else {
              // Simple prop definition: PropType or Array
              let type_text = get_node_text(&value_node, source);
              prop_info.prop_type = Some(type_text);
            }
          }

          state.props.push(prop_info);
        }
      }
    }
  }
}

/// Parses a complex prop definition object
fn parse_prop_definition(node: &Node, source: &str, prop_info: &mut PropInfo) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let key_text = get_node_text(&key_node, source);
          let key = key_text.trim_matches('"').trim_matches('\'');
          let value_text = get_node_text(&value_node, source);

          match key {
            "type" => {
              prop_info.prop_type = Some(value_text);
            }
            "required" => {
              prop_info.required = Some(value_text == "true");
            }
            "default" => {
              prop_info.default_value = Some(value_text);
            }
            "validator" => {
              prop_info.validator = Some(value_text);
            }
            _ => {}
          }
        }
      }
    }
  }
}

/// Parses the data function to extract data properties
fn parse_data_function(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Handle both data() { return { ... } } and data: () => ({ ... })
  if node.kind() == "function"
    || node.kind() == "function_expression"
    || node.kind() == "method_definition"
  {
    // Function syntax: data() { return { ... } }
    parse_data_function_body(node, source, state);
  } else if node.kind() == "arrow_function" {
    // Arrow function syntax: data: () => ({ ... })
    parse_data_arrow_function(node, source, state);
  } else {
    // General parsing for any other syntax
    parse_general_node(node, source, state);
  }
}

/// Parses the body of a data function to find return object
fn parse_data_function_body(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Look for return statement with object
  find_return_object(node, source, state);
}

/// Parses an arrow function data definition
fn parse_data_arrow_function(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // For arrow functions, the body might be directly an object or a block with return
  if let Some(body_node) = node.child_by_field_name("body") {
    if body_node.kind() == "object" {
      // Direct object: () => { prop: value }
      parse_data_object(&body_node, source, state);
    } else if body_node.kind() == "parenthesized_expression" {
      // Parenthesized object: () => ({ prop: value })
      // Look for the object inside the parentheses
      for i in 0..body_node.child_count() {
        if let Some(child) = body_node.child(i) {
          if child.kind() == "object" {
            parse_data_object(&child, source, state);
            break;
          }
        }
      }
    } else {
      // Block with return: () => { return { prop: value } }
      find_return_object(&body_node, source, state);
    }
  }
}

/// Recursively finds return statements with objects
fn find_return_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  if node.kind() == "return_statement" {
    // Find the object being returned
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "object" {
          parse_data_object(&child, source, state);
          return;
        }
      }
    }
  }

  // Continue searching in children
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      find_return_object(&child, source, state);
    }
  }
}

/// Parses a data object to extract property names and values
fn parse_data_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        if let Some(key_node) = child.child(0) {
          let prop_text = get_node_text(&key_node, source);
          let prop_name = prop_text.trim_matches('"').trim_matches('\'');

          let value = child
            .child(2)
            .map(|value_node| get_node_text(&value_node, source));

          state.data_properties.push(DataPropertyInfo {
            name: prop_name.to_string(),
            value,
          });
        }
      }
    }
  }
}

/// General node parser that extracts identifiers and function calls
fn parse_general_node(node: &Node, source: &str, state: &mut ScriptParsingState) {
  match node.kind() {
    "identifier" => {
      let identifier = get_node_text(node, source);
      if !identifier.is_empty() && !state.identifiers.contains(&identifier) {
        state.identifiers.push(identifier);
      }
    }
    "call_expression" => {
      // Extract function call details including parameters
      if let Some(call_detail) = extract_function_call_details(node, source) {
        // Add to function call details
        state.function_call_details.push(call_detail.clone());

        // Keep the existing simple function name for backward compatibility
        if !state.function_calls.contains(&call_detail.name) {
          state.function_calls.push(call_detail.name);
        }
      }

      // Continue parsing arguments
      for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
          parse_general_node(&child, source, state);
        }
      }
    }
    "member_expression" => {
      // Handle this.property patterns
      let member_text = get_node_text(node, source);
      if member_text.starts_with("this.") {
        let property = member_text.strip_prefix("this.").unwrap_or(&member_text);
        if !property.is_empty() && !state.identifiers.contains(&property.to_string()) {
          state.identifiers.push(property.to_string());
        }
      }

      // Continue parsing the member expression parts
      for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
          parse_general_node(&child, source, state);
        }
      }
    }
    _ => {
      // Recursively parse all children
      for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
          parse_general_node(&child, source, state);
        }
      }
    }
  }
}

/// Context containing all parsed information for transformation
#[derive(Debug, Clone)]
pub struct TransformationContext {
  pub script_state: ScriptParsingState,
  pub template_state: TemplateParsingState,
  pub sfc_sections: SfcSections,
}

/// Result of a transformation containing all changes to be applied
#[derive(Debug, Clone, Default)]
pub struct TransformationResult {
  pub imports_to_add: HashMap<String, Vec<String>>, // path => [import1, import2, ...]
  pub imports_to_remove: Vec<String>,
  pub setup: Vec<String>, // Composable setup (useStore, useRouter, etc.)
  pub reactive_state: Vec<String>, // ref() and reactive() declarations
  pub computed_properties: Vec<String>, // computed() declarations
  pub methods: Vec<String>, // Method definitions
  pub watchers: Vec<String>, // watch() and watchEffect() declarations
  pub lifecycle_hooks: Vec<String>, // onMounted, onBeforeUnmount, etc.
  pub template_replacements: Vec<TemplateReplacement>,
  pub additional_scripts: Vec<String>, // Additional script blocks to append
  pub skip_data_properties: Vec<String>, // Data properties to skip (handled by other transformers)
  pub data_refs: HashMap<String, (String, u8)>, // property_name => (ref_declaration, priority)
  pub resolved_identifiers: Vec<String>, // Identifiers that have been resolved by transformers
}

#[derive(Debug, Clone)]
pub struct TemplateReplacement {
  pub find: String,
  pub replace: String,
}

/// Configuration for transformers
#[derive(Debug, Clone, Default)]
pub struct TransformerConfig {
  pub enable_i18n: bool,
  pub enable_vuex_to_pinia: bool,
  pub enable_asset_transforms: bool,
  pub pinia_store_path: Option<String>,
  pub mixins: Option<HashMap<String, MixinConfig>>,
  pub imports_rewrite: Option<HashMap<String, ImportRewrite>>,
  pub additional_imports: Option<HashMap<String, AdditionalImport>>,
  pub import_keeplist: Option<Vec<String>>,
}

impl TransformationResult {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn merge(&mut self, other: TransformationResult) {
    // Merge imports by path, combining lists of imports for the same path
    for (path, imports) in other.imports_to_add {
      self.imports_to_add.entry(path).or_default().extend(imports);
    }
    self.imports_to_remove.extend(other.imports_to_remove);
    self.setup.extend(other.setup);
    self.reactive_state.extend(other.reactive_state);
    self.computed_properties.extend(other.computed_properties);
    self.methods.extend(other.methods);
    self.watchers.extend(other.watchers);
    self.lifecycle_hooks.extend(other.lifecycle_hooks);
    self
      .template_replacements
      .extend(other.template_replacements);
    self.additional_scripts.extend(other.additional_scripts);
    self.skip_data_properties.extend(other.skip_data_properties);
    self.resolved_identifiers.extend(other.resolved_identifiers);

    // Merge data refs by priority - higher priority overwrites lower priority
    for (prop_name, (ref_declaration, priority)) in other.data_refs {
      match self.data_refs.get(&prop_name) {
        Some((_, existing_priority)) if *existing_priority >= priority => {
          // Keep existing ref if it has higher or equal priority
        }
        _ => {
          // Override with new ref (higher priority or first time)
          self
            .data_refs
            .insert(prop_name, (ref_declaration, priority));
        }
      }
    }
  }

  /// Add an import item to a specific path
  pub fn add_import(&mut self, path: &str, import_item: &str) {
    self
      .imports_to_add
      .entry(path.to_string())
      .or_default()
      .push(import_item.to_string());
  }

  /// Add multiple import items to a specific path
  pub fn add_imports(&mut self, path: &str, import_items: &[&str]) {
    let imports = self.imports_to_add.entry(path.to_string()).or_default();
    imports.extend(import_items.iter().map(|s| s.to_string()));
  }

  /// Add content to the setup section (composable declarations, etc.)
  pub fn add_setup(&mut self, content: String) {
    self.setup.push(content);
  }

  /// Add content to the reactive state section (ref, reactive declarations)
  pub fn add_reactive_state(&mut self, content: String) {
    self.reactive_state.push(content);
  }

  /// Add content to the methods section
  pub fn add_method(&mut self, content: String) {
    self.methods.push(content);
  }

  /// Add content to the watchers section
  pub fn add_watcher(&mut self, content: String) {
    self.watchers.push(content);
  }

  /// Add content to the lifecycle hooks section
  pub fn add_lifecycle_hook(&mut self, content: String) {
    self.lifecycle_hooks.push(content);
  }

  /// Add content to computed properties section
  pub fn add_computed_property(&mut self, content: String) {
    self.computed_properties.push(content);
  }

  /// Backward compatibility: Add to setup_code (will be categorized automatically)
  pub fn add_setup_code(&mut self, content: String) {
    // For backward compatibility, add to the setup section by default
    // Individual transformers should migrate to use the specific methods
    self.setup.push(content);
  }

  /// Backward compatibility: Extend setup_code (will be categorized automatically)
  pub fn extend_setup_code(&mut self, content: Vec<String>) {
    // For backward compatibility, add to the setup section by default
    self.setup.extend(content);
  }

  /// Get all setup code in the correct order for assembly
  pub fn get_all_setup_code(&self) -> Vec<String> {
    let mut result = Vec::new();

    // 1. Composable setup first
    result.extend(self.setup.clone());

    // 2. Reactive state (ref/reactive declarations)
    result.extend(self.reactive_state.clone());

    // 3. Computed properties
    result.extend(self.computed_properties.clone());

    // 4. Methods
    result.extend(self.methods.clone());

    // 5. Watchers
    result.extend(self.watchers.clone());

    // 6. Lifecycle hooks last
    result.extend(self.lifecycle_hooks.clone());

    result
  }
}

/// Helper function to extract text content from a tree-sitter node
fn get_node_text(node: &Node, source: &str) -> String {
  source[node.start_byte()..node.end_byte()].to_string()
}

/// Check if a method is async by looking for the async keyword
fn check_if_async(node: &Node, source: &str) -> bool {
  // Check if the node itself contains 'async' keyword
  let node_text = get_node_text(node, source);
  node_text.contains("async")
}

/// Extract the method body from a function/method node
fn extract_method_body(node: &Node, source: &str) -> String {
  // For function expressions or method definitions, find the body
  if let Some(body_node) = node.child_by_field_name("body") {
    let body_text = get_node_text(&body_node, source);
    // Remove the outer braces and return the inner content
    if body_text.starts_with('{') && body_text.ends_with('}') {
      let inner = &body_text[1..body_text.len() - 1];
      inner.trim().to_string()
    } else {
      body_text
    }
  } else {
    // Fallback: try to find a statement_block or block_statement child
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "statement_block" || child.kind() == "block_statement" {
          let body_text = get_node_text(&child, source);
          if body_text.starts_with('{') && body_text.ends_with('}') {
            let inner = &body_text[1..body_text.len() - 1];
            return inner.trim().to_string();
          }
          return body_text;
        }
      }
    }
    // If no body found, return empty string
    String::new()
  }
}

/// Extract parameter names from a method/function definition
fn extract_method_parameters(node: &Node, source: &str) -> Vec<String> {
  let mut parameters = Vec::new();

  // Look for formal_parameters node
  if let Some(params_node) = node.child_by_field_name("parameters") {
    // Iterate through the parameters
    for i in 0..params_node.child_count() {
      if let Some(child) = params_node.child(i) {
        if child.kind() == "identifier" {
          let param_name = get_node_text(&child, source);
          parameters.push(param_name);
        }
      }
    }
  } else {
    // Fallback: look for parameters in child nodes
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "formal_parameters" {
          // Iterate through the parameters
          for j in 0..child.child_count() {
            if let Some(param_child) = child.child(j) {
              if param_child.kind() == "identifier" {
                let param_name = get_node_text(&param_child, source);
                parameters.push(param_name);
              }
            }
          }
          break;
        }
      }
    }
  }

  parameters
}

/// Extract parameter names from a watcher function definition
fn extract_watcher_param_names(node: &Node, source: &str) -> (String, String) {
  // Look for formal_parameters node
  if let Some(params_node) = node.child_by_field_name("parameters") {
    let mut param_names = Vec::new();

    // Iterate through the parameters
    for i in 0..params_node.child_count() {
      if let Some(child) = params_node.child(i) {
        if child.kind() == "identifier" {
          let param_name = get_node_text(&child, source);
          param_names.push(param_name);
        }
      }
    }

    // Return the first two parameter names, or defaults
    match param_names.len() {
      0 => ("newVal".to_string(), "oldVal".to_string()),
      1 => (param_names[0].clone(), "oldVal".to_string()),
      _ => (param_names[0].clone(), param_names[1].clone()),
    }
  } else {
    // Fallback: look for any formal_parameters child
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "formal_parameters" {
          let mut param_names = Vec::new();

          for j in 0..child.child_count() {
            if let Some(param_child) = child.child(j) {
              if param_child.kind() == "identifier" {
                let param_name = get_node_text(&param_child, source);
                param_names.push(param_name);
              }
            }
          }

          return match param_names.len() {
            0 => ("newVal".to_string(), "oldVal".to_string()),
            1 => (param_names[0].clone(), "oldVal".to_string()),
            _ => (param_names[0].clone(), param_names[1].clone()),
          };
        }
      }
    }

    // Default parameter names if we can't extract them
    ("newVal".to_string(), "oldVal".to_string())
  }
}

/// Iteratively walks a tree-sitter AST to extract identifiers and function calls for template parsing.
pub fn walk_tree_recursive_template(
  node: tree_sitter::Node,
  source: &[u8],
  state: &mut TemplateParsingState,
) {
  // Process current node
  match node.kind() {
    "identifier" => {
      if let Ok(text) = node.utf8_text(source) {
        if !state.identifiers.contains(&text.to_string()) {
          state.identifiers.push(text.to_string());
        }
      }
    },
    "call_expression" => {
      // Extract detailed function call information
      if let Some(function_name) = node.child_by_field_name("function") {
        if let Ok(function_text) = function_name.utf8_text(source) {
          // Get full call text
          if let Ok(full_call_text) = node.utf8_text(source) {
            // Extract arguments from the call
            if let Some(arguments_node) = node.child_by_field_name("arguments") {
              let mut arguments = Vec::new();

              // Parse each argument
              for i in 0..arguments_node.child_count() {
                if let Some(child) = arguments_node.child(i) {
                  if child.kind() != "," {
                    if let Ok(arg_text) = child.utf8_text(source) {
                      arguments.push(arg_text.to_string());
                    }
                  }
                }
              }

              let call_detail = FunctionCallDetail {
                name: function_text.to_string(),
                arguments,
                full_call: full_call_text.to_string(),
              };

              state.function_call_details.push(call_detail.clone());

              // Keep the existing simple function name for backward compatibility
              if !state.function_calls.contains(&call_detail.name) {
                state.function_calls.push(call_detail.name);
              }
            }
          }
        }
      }
    },
    _ => {} // Process other node types if needed in the future
  }

  // Recursively process all child nodes to ensure we visit every node in the tree
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      walk_tree_recursive_template(child, source, state);
    }
  }
}

/// Parses a Vue template section using lol_html and tree-sitter to extract Vue directives and mustache expressions.
///
/// This function analyzes the HTML template within a Vue component and identifies:
/// - **Vue Directives**: Attributes starting with "v-" or ":" (e.g., v-for, :href, @click)
/// - **Mustache Expressions**: Template interpolations using {{ }} syntax
/// - **Identifiers and Function Calls**: Within directive values and mustache expressions using tree-sitter
///
/// The parsing is performed using lol_html for HTML parsing and tree-sitter for JavaScript expression analysis.
///
/// # Arguments
///
/// * `template_content` - The HTML content from within the `<template>` tag
/// * `state` - Mutable reference to the template parsing state that accumulates results
///
/// # Returns
///
/// Returns `Result<(), Box<dyn std::error::Error>>` indicating success or parsing failure.
///
/// # Examples
///
/// ```
/// use vue_options_to_composition::{parse_template_section, TemplateParsingState};
///
/// let template = r#"
/// <div class="container">
///   <h1>{{ title }}</h1>
///   <button @click="handleClick">{{ buttonText }}</button>
///   <div v-for="item in items" :key="item.id">
///     <span :title="item.tooltip">{{ item.name }}</span>
///   </div>
///   <input v-model="searchQuery" :placeholder="$t('search.placeholder')" />
/// </div>
/// "#;
///
/// let mut state = TemplateParsingState::new();
/// parse_template_section(template, &mut state).unwrap();
///
/// // Check that Vue directives were identified
/// assert!(state.vue_directives.iter().any(|d| d.name == "v-for"));
/// assert!(state.vue_directives.iter().any(|d| d.name == ":key"));
/// assert!(state.vue_directives.iter().any(|d| d.name == "@click"));
/// assert!(state.vue_directives.iter().any(|d| d.name == "v-model"));
///
/// // Check that mustache expressions were identified
/// assert!(state.mustache_expressions.iter().any(|e| e.content == "title"));
/// assert!(state.mustache_expressions.iter().any(|e| e.content == "buttonText"));
/// assert!(state.mustache_expressions.iter().any(|e| e.content == "item.name"));
///
/// // Check that identifiers and function calls were parsed
/// assert!(state.identifiers.contains(&"title".to_string()));
/// assert!(state.identifiers.contains(&"items".to_string()));
/// assert!(state.function_calls.contains(&"$t".to_string()));
/// ```
pub fn parse_template_section(
  template_content: &str,
  state: &mut TemplateParsingState,
) -> Result<(), Box<dyn std::error::Error>> {
  use lol_html::{doc_text, element, rewrite_str, RewriteStrSettings};
  use std::sync::{Arc, Mutex};

  // Use Arc<Mutex<Vec<_>>> to collect results from closures
  let temp_directives = Arc::new(Mutex::new(Vec::new()));
  let temp_mustaches = Arc::new(Mutex::new(Vec::new()));

  // Parse Vue directives and attributes
  let directives_ref = Arc::clone(&temp_directives);
  let element_content_handlers = vec![element!("*", move |el| {
    let tag_name = el.tag_name();
    let vue_attributes = el.attributes().iter().filter(|attr| {
      let name = attr.name();
      name.starts_with("v-") || name.starts_with(":") || name.starts_with("@")
    });

    for attr in vue_attributes {
      let attr_name = attr.name();
      let attr_value = attr.value();

      directives_ref.lock().unwrap().push(VueDirectiveInfo {
        name: attr_name.to_string(),
        value: attr_value.to_string(),
        element_tag: tag_name.to_string(),
      });
    }

    Ok(())
  })];

  // Parse mustache expressions
  let mustaches_ref = Arc::clone(&temp_mustaches);
  let document_content_handlers = vec![doc_text!(move |t| {
    let mustache_regex = &*MUSTACHE_PATTERN;
    for cap in mustache_regex.captures_iter(t.as_str()) {
      let mustache_content = cap.get(1).map_or("", |m| m.as_str()).trim();

      mustaches_ref.lock().unwrap().push(MustacheExpressionInfo {
        content: mustache_content.to_string(),
      });
    }

    Ok(())
  })];

  // Process the template HTML
  let _output = rewrite_str(
    template_content,
    RewriteStrSettings {
      element_content_handlers,
      document_content_handlers,
      ..RewriteStrSettings::new()
    },
  )?;

  // Now parse collected directives and mustaches with tree-sitter
  let language = tree_sitter_javascript::LANGUAGE.into();
  let mut parser = Parser::new();
  parser.set_language(&language)?;

  // Process directives
  let directives = temp_directives.lock().unwrap();
  for directive in directives.iter() {
    state.vue_directives.push(directive.clone());

    if let Some(tree) = parser.parse(directive.value.as_bytes(), None) {
      let root_node = tree.root_node();
      walk_tree_recursive_template(root_node, directive.value.as_bytes(), state);
    }
  }

  // Process mustache expressions
  let mustaches = temp_mustaches.lock().unwrap();
  for mustache in mustaches.iter() {
    state.mustache_expressions.push(mustache.clone());

    if let Some(tree) = parser.parse(mustache.content.as_bytes(), None) {
      let root_node = tree.root_node();
      walk_tree_recursive_template(root_node, mustache.content.as_bytes(), state);
    }
  }

  Ok(())
}

/// Parses the head method to extract its body for transformation to useHead
fn parse_head_method(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Extract head method details
  let is_async = check_if_async(node, source);
  let body = extract_method_body(node, source);

  state.head_method = Some(MethodDetail {
    name: "head".to_string(),
    parameters: Vec::new(), // head() method has no parameters
    body,
    is_async,
  });

  // Also parse for general identifiers and function calls
  parse_general_node(node, source, state);
}

/// Parses the fetch method to extract its body for transformation to useFetch
fn parse_fetch_method(node: &Node, source: &str, state: &mut ScriptParsingState) {
  // Extract fetch method details
  let is_async = check_if_async(node, source);
  let body = extract_method_body(node, source);

  state.fetch_method = Some(MethodDetail {
    name: "fetch".to_string(),
    parameters: Vec::new(), // fetch() method typically has no parameters
    body,
    is_async,
  });

  // Also parse for general identifiers and function calls
  parse_general_node(node, source, state);
}

/// Parses a watchers object to extract individual watcher definitions
fn parse_watchers_object(node: &Node, source: &str, state: &mut ScriptParsingState) {
  for i in 0..node.child_count() {
    if let Some(child) = node.child(i) {
      if child.kind() == "pair" {
        // Handle watcher: function(newVal, oldVal) { ... } syntax
        if let (Some(key_node), Some(value_node)) = (child.child(0), child.child(2)) {
          let watcher_text = get_node_text(&key_node, source);
          let watched_property = watcher_text.trim_matches('"').trim_matches('\'');

          let is_async = check_if_async(&value_node, source);
          let handler_body = extract_method_body(&value_node, source);
          let param_names = extract_watcher_param_names(&value_node, source);

          state.watchers.push(WatcherDetail {
            watched_property: watched_property.to_string(),
            handler_body,
            is_async,
            param_names,
          });
        }
      } else if child.kind() == "method_definition" {
        // Handle shorthand watcher syntax: watchedProperty(newVal, oldVal) { ... }
        if let Some(name_node) = child.child_by_field_name("name") {
          let watcher_text = get_node_text(&name_node, source);
          let watched_property = watcher_text.trim_matches('"').trim_matches('\'');

          let is_async = check_if_async(&child, source);
          let handler_body = extract_method_body(&child, source);
          let param_names = extract_watcher_param_names(&child, source);

          state.watchers.push(WatcherDetail {
            watched_property: watched_property.to_string(),
            handler_body,
            is_async,
            param_names,
          });
        }
      }
    }
  }

  // Also parse for general identifiers and function calls
  parse_general_node(node, source, state);
}

/// Information about a function call with its parameters.
#[derive(Debug, Clone)]
pub struct FunctionCallDetail {
  pub name: String,
  pub arguments: Vec<String>,
  pub full_call: String,
}

/// Extract function call details including arguments from a call_expression node
fn extract_function_call_details(node: &Node, source: &str) -> Option<FunctionCallDetail> {
  if node.kind() != "call_expression" {
    return None;
  }

  // Get the function name
  let function_node = node.child_by_field_name("function")?;
  let function_name = get_node_text(&function_node, source);

  // Get the arguments
  let arguments_node = node.child_by_field_name("arguments")?;
  let mut arguments = Vec::new();

  // Parse each argument
  for i in 0..arguments_node.child_count() {
    if let Some(child) = arguments_node.child(i) {
      if child.kind() != "," {
        let arg_text = get_node_text(&child, source);
        arguments.push(arg_text);
      }
    }
  }

  // Get the full call text
  let full_call = get_node_text(node, source);

  Some(FunctionCallDetail {
    name: function_name,
    arguments,
    full_call,
  })
}
