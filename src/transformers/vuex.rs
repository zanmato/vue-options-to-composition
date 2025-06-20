use super::Transformer;
use crate::{
  FunctionCallDetail, TemplateReplacement, TransformationContext, TransformationResult,
  TransformerConfig,
};
use std::collections::HashSet;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref VUEX_NAMESPACE_PATTERN: Regex = Regex::new(r#"['`"]([a-zA-Z_]\w*)/[a-zA-Z_]\w*['`"]"#).unwrap();
    static ref VUEX_MAP_NS_PATTERN: Regex = Regex::new(
        r#"map(?:State|Getters|Mutations|Actions)\(\s*['"`]([a-zA-Z_]\w*)['"`]\s*,\s*\["#
    ).unwrap();
    static ref VUEX_COMMIT_PATTERN: Regex = Regex::new(r#"this\.\$store\.commit\(['"]([a-zA-Z_]\w*)/([a-zA-Z_]\w*)['"](?:,\s*([^)]+))?\)"#).unwrap();
    static ref VUEX_DISPATCH_PATTERN: Regex = Regex::new(r#"this\.\$store\.dispatch\(['"]([a-zA-Z_]\w*)/([a-zA-Z_]\w*)['"](?:,\s*([^)]+))?\)"#).unwrap();
    static ref VUEX_STATE_PATTERN: Regex = Regex::new(r#"this\.\$store\.state\.([a-zA-Z_]\w*)\.([a-zA-Z_]\w*)"#).unwrap();
    static ref VUEX_TEMPLATE_STATE_PATTERN: Regex = Regex::new(r#"\$store\.state\.([a-zA-Z_]\w*)\.([a-zA-Z_]\w*)"#).unwrap();
}

/// Transformer for Vuex to Pinia store usage
///
/// This transformer handles:
/// - Converting `this.$store.commit('namespace/action')` to `namespaceStore.action()`
/// - Converting `this.$store.dispatch('namespace/action')` to `namespaceStore.action()`
/// - Converting `this.$store.state.namespace.property` to `namespaceStore.property`
/// - Adding appropriate Pinia store imports
/// - Extracting namespaces from Vuex usage patterns
pub struct VuexTransformer;

impl Default for VuexTransformer {
  fn default() -> Self {
    Self::new()
  }
}

impl VuexTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Extract store namespaces used in the component
  fn extract_store_namespaces(&self, context: &TransformationContext) -> HashSet<String> {
    let mut namespaces = HashSet::new();

    // Extract namespaces from script function call details
    for function_call in &context.script_state.function_call_details {
      self.extract_namespaces_from_function_call(function_call, &mut namespaces);
    }

    // Extract namespaces from template function call details
    for function_call in &context.template_state.function_call_details {
      self.extract_namespaces_from_function_call(function_call, &mut namespaces);
    }

    // Extract namespaces from identifiers (for $store.state.namespace patterns)
    for identifier in &context.script_state.identifiers {
      self.extract_namespaces_from_identifier(identifier, &mut namespaces);
    }

    for identifier in &context.template_state.identifiers {
      self.extract_namespaces_from_identifier(identifier, &mut namespaces);
    }

    // Extract namespaces from Vuex map functions if needed (fallback)
    if let Some(script_content) = &context.sfc_sections.script_content {
      self.extract_namespaces_from_map_functions(script_content, &mut namespaces);
    }

    namespaces
  }

  /// Extract namespaces from a function call detail
  fn extract_namespaces_from_function_call(
    &self,
    function_call: &FunctionCallDetail,
    namespaces: &mut HashSet<String>,
  ) {
    // Check for $store.commit or $store.dispatch patterns with namespace/action arguments
    if function_call.name.contains("$store.commit")
      || function_call.name.contains("$store.dispatch")
    {
      for arg in &function_call.arguments {
        // Look for 'namespace/action' pattern in string arguments
        if let Some(namespace) = self.extract_namespace_from_string_arg(arg) {
          namespaces.insert(namespace);
        }
      }
    }

    // Check for mapGetters, mapActions, mapMutations with namespace/action patterns in object values
    if function_call.name == "mapGetters"
      || function_call.name == "mapActions"
      || function_call.name == "mapMutations"
    {
      for arg in &function_call.arguments {
        // Look for object containing 'namespace/action' patterns
        self.extract_namespaces_from_object_arg(arg, namespaces);
      }
    }
  }

  /// Extract namespaces from object argument like { user: 'user/getUser', hasGrants: 'cart/hasGrants' }
  fn extract_namespaces_from_object_arg(&self, arg: &str, namespaces: &mut HashSet<String>) {
    // Look for 'namespace/action' patterns inside the object string
    // This is a simple string-based extraction since the arg contains the full object text
    let namespace_pattern = &*VUEX_NAMESPACE_PATTERN;
    for captures in namespace_pattern.captures_iter(arg) {
      if let Some(namespace) = captures.get(1) {
        namespaces.insert(namespace.as_str().to_string());
      }
    }
  }

  /// Extract namespaces from an identifier
  fn extract_namespaces_from_identifier(&self, identifier: &str, namespaces: &mut HashSet<String>) {
    // Pattern: $store.state.namespace.property or this.$store.state.namespace.property
    if identifier.contains("$store.state.") {
      if let Some(namespace) = self.extract_namespace_from_state_access(identifier) {
        namespaces.insert(namespace);
      }
    }
  }

  /// Extract namespace from a string argument like 'namespace/action' or "namespace/action"
  fn extract_namespace_from_string_arg(&self, arg: &str) -> Option<String> {
    // Remove quotes and look for namespace/action pattern
    let cleaned = arg.trim_matches(|c| c == '\'' || c == '"' || c == '`');
    if let Some(slash_pos) = cleaned.find('/') {
      let namespace = &cleaned[..slash_pos];
      if !namespace.is_empty() && namespace.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Some(namespace.to_string());
      }
    }
    None
  }

  /// Extract namespace from state access pattern like $store.state.namespace.property
  fn extract_namespace_from_state_access(&self, identifier: &str) -> Option<String> {
    // Look for pattern: [$store.state.]namespace[.property]
    if let Some(state_pos) = identifier.find("$store.state.") {
      let after_state = &identifier[state_pos + "$store.state.".len()..];
      if let Some(dot_pos) = after_state.find('.') {
        let namespace = &after_state[..dot_pos];
        if !namespace.is_empty() && namespace.chars().all(|c| c.is_alphanumeric() || c == '_') {
          return Some(namespace.to_string());
        }
      } else {
        // Handle case where there's no property after namespace
        let namespace = after_state;
        if !namespace.is_empty() && namespace.chars().all(|c| c.is_alphanumeric() || c == '_') {
          return Some(namespace.to_string());
        }
      }
    }
    None
  }

  /// Extract namespaces from Vuex map functions
  fn extract_namespaces_from_map_functions(
    &self,
    script_content: &str,
    namespaces: &mut HashSet<String>,
  ) {
    // Pattern to find any namespace/action pattern in quotes
    let namespace_pattern = &*VUEX_NAMESPACE_PATTERN;
    for captures in namespace_pattern.captures_iter(script_content) {
      if let Some(namespace) = captures.get(1) {
        namespaces.insert(namespace.as_str().to_string());
      }
    }

    // Pattern for mapState('namespace', { ... }) and other map functions with namespace as first argument
    let map_ns_pattern = &*VUEX_MAP_NS_PATTERN;
    for captures in map_ns_pattern.captures_iter(script_content) {
      if let Some(namespace) = captures.get(1) {
        namespaces.insert(namespace.as_str().to_string());
      }
    }
  }

  /// Get body transformation function for converting store calls and computed properties
  fn get_vuex_body_transform(
  ) -> Box<dyn Fn(&str, &TransformationContext, &TransformerConfig) -> String> {
    Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let mut transformed_body = body.to_string();

        // Create a temporary VuexTransformer instance to access methods
        let transformer = VuexTransformer::new();

        // Transform computed properties created by Vuex map functions to .value syntax
        let aliased_getters = transformer.extract_aliased_getters(context);
        for (alias, _namespace, _getter, _is_array_syntax) in &aliased_getters {
          let pattern = format!("\\bthis\\.{}\\b", regex::escape(alias));
          if let Ok(regex_pattern) = regex::Regex::new(&pattern) {
            transformed_body = regex_pattern
              .replace_all(&transformed_body, format!("{}.value", alias))
              .to_string();
          }
        }

        let aliased_state = transformer.extract_aliased_state_properties(context);
        for (alias, _namespace, _property, _is_array_syntax) in &aliased_state {
          let pattern = format!("\\bthis\\.{}\\b", regex::escape(alias));
          if let Ok(regex_pattern) = regex::Regex::new(&pattern) {
            transformed_body = regex_pattern
              .replace_all(&transformed_body, format!("{}.value", alias))
              .to_string();
          }
        }

        // Extract aliased methods from map functions
        let aliased_actions = transformer.extract_aliased_actions(context);
        let aliased_mutations = transformer.extract_aliased_mutations(context);
        let _aliased_getters = transformer.extract_aliased_getters(context);
        let _aliased_state = transformer.extract_aliased_state_properties(context);

        // Transform calls to aliased actions: this.fetchUser() -> userStore.fetchUser()
        for (alias, namespace, _action_name, _is_array_syntax) in &aliased_actions {
          let pattern = format!("this\\.{}\\(", regex::escape(alias));
          if let Ok(regex_pattern) = regex::Regex::new(&pattern) {
            transformed_body = regex_pattern
              .replace_all(&transformed_body, format!("{}Store.{}(", namespace, alias))
              .to_string();
          }
        }

        // Transform calls to aliased mutations: this.setUser() -> userStore.setUser()
        for (alias, namespace, _mutation_name, _is_array_syntax) in &aliased_mutations {
          let pattern = format!("this\\.{}\\(", regex::escape(alias));
          if let Ok(regex_pattern) = regex::Regex::new(&pattern) {
            transformed_body = regex_pattern
              .replace_all(&transformed_body, format!("{}Store.{}(", namespace, alias))
              .to_string();
          }
        }

        // Note: We don't transform aliased getters (this.user) here because they become
        // computed properties that are handled by the composition transformer
        // (this.user -> user.value, not userStore.getUser)

        // Note: We don't transform aliased state (this.userID) here because they become
        // computed properties that are handled by the composition transformer
        // (this.userID -> userID.value, not userStore.userID)

        // Transform commit calls: this.$store.commit('namespace/action', args) -> namespaceStore.action(args)
        let commit_pattern = &*VUEX_COMMIT_PATTERN;
        transformed_body = commit_pattern
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let namespace = &caps[1];
            let action = &caps[2];
            let args = caps.get(3).map_or("", |m| m.as_str());

            if args.is_empty() {
              format!("{}Store.{}()", namespace, action)
            } else {
              format!("{}Store.{}({})", namespace, action, args)
            }
          })
          .to_string();

        // Transform dispatch calls: this.$store.dispatch('namespace/action', args) -> namespaceStore.action(args)
        let dispatch_pattern = &*VUEX_DISPATCH_PATTERN;
        transformed_body = dispatch_pattern
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let namespace = &caps[1];
            let action = &caps[2];
            let args = caps.get(3).map_or("", |m| m.as_str());

            if args.is_empty() {
              format!("{}Store.{}()", namespace, action)
            } else {
              format!("{}Store.{}({})", namespace, action, args)
            }
          })
          .to_string();

        // Transform state access: this.$store.state.namespace.property -> namespaceStore.property
        let state_pattern = &*VUEX_STATE_PATTERN;
        transformed_body = state_pattern
          .replace_all(&transformed_body, |caps: &regex::Captures| {
            let namespace = &caps[1];
            let property = &caps[2];
            format!("{}Store.{}", namespace, property)
          })
          .to_string();

        transformed_body
      },
    )
  }

  /// Generate template replacements for $store usage
  fn generate_template_replacements(
    &self,
    context: &TransformationContext,
  ) -> Vec<TemplateReplacement> {
    let mut replacements = Vec::new();

    if let Some(template_content) = &context.sfc_sections.template_content {
      // Replace $store.state.namespace.property with namespaceStore.property
      let state_pattern = &*VUEX_TEMPLATE_STATE_PATTERN;

      for captures in state_pattern.captures_iter(template_content) {
        if let (Some(namespace_match), Some(property_match)) = (captures.get(1), captures.get(2)) {
          let namespace = namespace_match.as_str();
          let property = property_match.as_str();
          let full_match = captures.get(0).unwrap().as_str();

          replacements.push(TemplateReplacement {
            find: full_match.to_string(),
            replace: format!("{}Store.{}", namespace, property),
          });
        }
      }
    }

    replacements
  }

  /// Check if the component uses Vuex store
  fn has_vuex_usage(&self, context: &TransformationContext) -> bool {
    // Check if there are any vuex_identifiers in the script
    context
      .script_state
      .function_calls
      .iter()
      .any(|call| call.contains("$store"))
      || context
        .template_state
        .identifiers
        .iter()
        .any(|id| id.contains("$store"))
      || {
        context.script_state.identifiers.iter().any(|id| {
          id.contains("$store")
            || id.contains("mapState")
            || id.contains("mapGetters")
            || id.contains("mapActions")
            || id.contains("mapMutations")
        })
      }
  }

  /// Extract aliased getters and state properties from map function calls
  /// Returns Vec<(alias, namespace, getter_or_property_name, is_array_syntax)>
  fn extract_aliased_getters(
    &self,
    context: &TransformationContext,
  ) -> Vec<(String, String, String, bool)> {
    let mut aliased_getters = Vec::new();

    for function_call in &context.script_state.function_call_details {
      if function_call.name == "mapGetters" {
        // Parse using tree-sitter: mapGetters({ alias: 'namespace/action' }) or mapGetters('namespace', ['getUser'])
        self.parse_map_function_with_tree_sitter(&function_call.full_call, &mut aliased_getters);
      }
    }

    aliased_getters
  }

  /// Extract aliased actions from map function calls
  /// Returns Vec<(alias, namespace, action_name, is_array_syntax)>
  fn extract_aliased_actions(
    &self,
    context: &TransformationContext,
  ) -> Vec<(String, String, String, bool)> {
    let mut aliased_actions = Vec::new();

    for function_call in &context.script_state.function_call_details {
      if function_call.name == "mapActions" {
        // Parse using tree-sitter: mapActions({ alias: 'namespace/action' }) or mapActions('namespace', ['fetchUser'])
        self.parse_map_function_with_tree_sitter(&function_call.full_call, &mut aliased_actions);
      }
    }

    aliased_actions
  }

  /// Extract aliased mutations from map function calls
  /// Returns Vec<(alias, namespace, mutation_name, is_array_syntax)>
  fn extract_aliased_mutations(
    &self,
    context: &TransformationContext,
  ) -> Vec<(String, String, String, bool)> {
    let mut aliased_mutations = Vec::new();

    for function_call in &context.script_state.function_call_details {
      if function_call.name == "mapMutations" {
        // Parse using tree-sitter: mapMutations({ alias: 'namespace/mutation' }) or mapMutations('namespace', ['updateUser'])
        self.parse_map_function_with_tree_sitter(&function_call.full_call, &mut aliased_mutations);
      }
    }

    aliased_mutations
  }

  /// Parse map function calls using tree-sitter
  fn parse_map_function_with_tree_sitter(
    &self,
    full_call: &str,
    results: &mut Vec<(String, String, String, bool)>,
  ) {
    let mut parser = tree_sitter::Parser::new();
    parser
      .set_language(&tree_sitter_javascript::LANGUAGE.into())
      .expect("Error loading JavaScript grammar");

    if let Some(tree) = parser.parse(full_call, None) {
      let root_node = tree.root_node();
      // Use unified parsing for all map functions
      self.extract_from_map_call(&root_node, full_call, results);
    }
  }

  /// Extract from map calls: mapGetters({ alias: 'namespace/action' }) or mapGetters('namespace', ['action'])
  fn extract_from_map_call(
    &self,
    node: &tree_sitter::Node,
    source: &str,
    results: &mut Vec<(String, String, String, bool)>,
  ) {
    if node.kind() == "call_expression" {
      if let Some(arguments) = node.child_by_field_name("arguments") {
        let mut namespace: Option<String> = None;
        let mut second_arg: Option<tree_sitter::Node> = None;

        // Check if this is namespace-first syntax: mapGetters('namespace', [...])
        let mut first_string_arg: Option<tree_sitter::Node> = None;
        let mut array_or_object_arg: Option<tree_sitter::Node> = None;

        for i in 0..arguments.child_count() {
          if let Some(child) = arguments.child(i) {
            match child.kind() {
              "string" => {
                if first_string_arg.is_none() {
                  first_string_arg = Some(child);
                  let arg_text = self.get_node_text(&child, source);
                  let cleaned = arg_text.trim_matches('\'').trim_matches('"');
                  // If it doesn't contain '/', it's likely a namespace
                  if !cleaned.contains('/') {
                    namespace = Some(cleaned.to_string());
                  }
                }
              }
              "object" | "array" => {
                if array_or_object_arg.is_none() {
                  array_or_object_arg = Some(child);
                }
              }
              _ => {
                // Skip punctuation and other nodes
              }
            }
          }
        }

        // Determine the second argument based on what we found
        if array_or_object_arg.is_some() {
          second_arg = array_or_object_arg;
        }

        if let Some(arg) = second_arg {
          if arg.kind() == "object" {
            self.extract_object_mappings(&arg, source, results, namespace, false);
          } else if arg.kind() == "array" && namespace.is_some() {
            self.extract_array_mappings(&arg, source, results, namespace, true);
          }
        }
      }
    }

    // Recursively search child nodes
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        self.extract_from_map_call(&child, source, results);
      }
    }
  }

  /// Extract key-value pairs from an object node
  fn extract_object_mappings(
    &self,
    node: &tree_sitter::Node,
    source: &str,
    results: &mut Vec<(String, String, String, bool)>,
    namespace: Option<String>,
    is_array_syntax: bool,
  ) {
    for i in 0..node.child_count() {
      if let Some(child) = node.child(i) {
        if child.kind() == "pair" {
          let mut key: Option<String> = None;
          let mut value: Option<String> = None;

          for j in 0..child.child_count() {
            if let Some(grandchild) = child.child(j) {
              match grandchild.kind() {
                "property_identifier" => {
                  key = Some(self.get_node_text(&grandchild, source));
                }
                "string" => {
                  let string_content = self.get_node_text(&grandchild, source);
                  value = Some(
                    string_content
                      .trim_matches('\'')
                      .trim_matches('"')
                      .to_string(),
                  );
                }
                _ => {}
              }
            }
          }

          if let (Some(alias), Some(val)) = (key, value) {
            if let Some(ref ns) = namespace {
              // mapState case: namespace provided, value is the property name
              results.push((alias, ns.clone(), val, is_array_syntax));
            } else {
              // mapGetters/mapActions/mapMutations case: value is 'namespace/action'
              if let Some(slash_pos) = val.find('/') {
                let ns = val[..slash_pos].to_string();
                let action = val[slash_pos + 1..].to_string();
                results.push((alias, ns, action, is_array_syntax));
              }
            }
          }
        }
      }
    }
  }

  /// Extract array elements from an array node for namespace-first syntax
  /// For mapState('user', ['userID']) -> [(userID, user, userID)]
  /// For mapGetters('user', ['getUser']) -> [(user, user, getUser)] (remove 'get' prefix for alias)
  fn extract_array_mappings(
    &self,
    node: &tree_sitter::Node,
    source: &str,
    results: &mut Vec<(String, String, String, bool)>,
    namespace: Option<String>,
    is_array_syntax: bool,
  ) {
    if let Some(ns) = namespace {
      for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
          if child.kind() == "string" {
            let string_content = self.get_node_text(&child, source);
            let property_name = string_content
              .trim_matches('\'')
              .trim_matches('"')
              .to_string();

            // For getters, derive alias by removing 'get' prefix if present
            let alias = if property_name.starts_with("get") && property_name.len() > 3 {
              // Convert "getUser" to "user"
              let without_get = &property_name[3..];
              format!(
                "{}{}",
                without_get.chars().next().unwrap().to_lowercase(),
                &without_get[1..]
              )
            } else {
              // For state, actions, mutations, use property name as alias
              property_name.clone()
            };

            results.push((alias, ns.clone(), property_name, is_array_syntax));
          }
        }
      }
    }
  }

  /// Helper to get text content from a tree-sitter node
  fn get_node_text(&self, node: &tree_sitter::Node, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
  }

  /// Check if a property is actually used in the template or script
  fn is_property_used(&self, property_name: &str, context: &TransformationContext) -> bool {
    // Check template usage
    if let Some(template_content) = &context.sfc_sections.template_content {
      if template_content.contains(&format!("{{ {}", property_name))
        || template_content.contains(&format!("{{{{{}", property_name))
        || template_content.contains(&format!(" {}", property_name))
        || template_content.contains(&format!("\"{}\"", property_name))
        || template_content.contains(&format!("'{}'", property_name))
      {
        return true;
      }
    }

    // Check script usage - look for this.propertyName usage
    if let Some(script_content) = &context.sfc_sections.script_content {
      if script_content.contains(&format!("this.{}", property_name)) {
        return true;
      }
    }

    // Check identifiers in script state
    for identifier in &context.script_state.identifiers {
      if identifier == property_name || identifier == &format!("this.{}", property_name) {
        return true;
      }
    }

    // Check identifiers in template state
    for identifier in &context.template_state.identifiers {
      if identifier == property_name {
        return true;
      }
    }

    false
  }

  /// Extract aliased state properties from mapState function calls
  /// Returns Vec<(alias, namespace, state_property, is_array_syntax)>
  fn extract_aliased_state_properties(
    &self,
    context: &TransformationContext,
  ) -> Vec<(String, String, String, bool)> {
    let mut aliased_state = Vec::new();

    for function_call in &context.script_state.function_call_details {
      if function_call.name == "mapState" {
        // Parse using tree-sitter: mapState('namespace', { alias: 'property' }) or mapState('namespace', ['userID'])
        self.parse_map_function_with_tree_sitter(&function_call.full_call, &mut aliased_state);
      }
    }

    aliased_state
  }
}

impl Transformer for VuexTransformer {
  fn name(&self) -> &'static str {
    "vuex"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_vuex_usage(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::new();

    // Extract store namespaces used in the component
    let namespaces = self.extract_store_namespaces(context);

    // Also extract namespaces from template usage (already handled in extract_store_namespaces)
    // No additional template extraction needed as it's now included in extract_store_namespaces

    if !namespaces.is_empty() {
      // Generate store imports and setup
      let mut sorted_namespaces: Vec<String> = namespaces.into_iter().collect();
      sorted_namespaces.sort(); // Sort for consistent output

      for namespace in &sorted_namespaces {
        // Add store import
        result.add_import(
          &format!("@/stores/{}", namespace),
          &format!("use{}Store", capitalize_first_letter(namespace)),
        );

        // Add store setup
        result.setup.push(format!(
          "const {}Store = use{}Store();",
          namespace,
          capitalize_first_letter(namespace)
        ));
      }

      if !result.setup.is_empty() {
        result.setup.push("".to_string()); // Empty line for readability
      }
    }

    // Generate computed properties from mapGetters (only if used)
    let aliased_getters = self.extract_aliased_getters(context);
    for (alias, namespace, getter, is_array_syntax) in aliased_getters {
      if self.is_property_used(&alias, context) {
        let parentheses = if is_array_syntax { "()" } else { "" };
        result.computed_properties.push(format!(
          "const {} = computed(() => {}Store.{}{});",
          alias, namespace, getter, parentheses
        ));
      }
    }

    // Generate computed properties from mapState (only if used)
    let aliased_state = self.extract_aliased_state_properties(context);
    for (alias, namespace, property, _is_array_syntax) in aliased_state {
      if self.is_property_used(&alias, context) {
        result.computed_properties.push(format!(
          "const {} = computed(() => {}Store.{});",
          alias, namespace, property
        ));
      }
    }

    // Generate template replacements for $store usage
    result
      .template_replacements
      .extend(self.generate_template_replacements(context));

    // Remove Vuex imports since we're converting to Pinia
    result.imports_to_remove.push("vuex".to_string());

    result
  }

  fn get_body_transform(&self) -> Option<Box<super::BodyTransformFn>> {
    Some(Self::get_vuex_body_transform())
  }
}

/// Capitalize the first letter of a string
fn capitalize_first_letter(s: &str) -> String {
  let mut chars = s.chars();
  match chars.next() {
    None => String::new(),
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
  }
}
