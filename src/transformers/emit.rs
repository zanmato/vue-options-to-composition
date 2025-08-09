use super::{BodyTransformFn, Transformer};
use crate::{TransformationContext, TransformationResult, TransformerConfig};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref EMIT_TEMPLATE_PATTERN: Regex = Regex::new(r#"\$emit\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap();
    static ref EMIT_THIS_PATTERN: Regex = Regex::new(r#"this\.\$emit\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap();
}

/// Transformer for converting Vue2 $emit usage to Vue3 defineEmits pattern
///
/// This transformer handles the conversion of:
/// - `this.$emit('event', data)` -> `emit('event', data)`
/// - Generates `const emit = defineEmits(['event1', 'event2']);`
/// - Maps Vue2 event names to Vue3 equivalents (e.g., 'input' -> 'update:value')
pub struct EmitTransformer;

impl Default for EmitTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl EmitTransformer {
  pub fn new() -> Self {
    Self
  }

  /// Check if context contains $emit usage
  fn has_emit_usage(&self, context: &TransformationContext) -> bool {
    self.has_emit_in_identifiers(context) || self.has_emit_in_methods(context) || self.has_emit_in_computed(context) || self.has_emit_in_template(context)
  }

  /// Check for $emit usage in template
  fn has_emit_in_template(&self, context: &TransformationContext) -> bool {
    // Check template function calls and identifiers for $emit
    context
      .template_state
      .function_calls
      .iter()
      .any(|call| call.contains("$emit") && !call.contains("$nuxt.$emit"))
      || context
        .template_state
        .identifiers
        .iter()
        .any(|id| id.contains("$emit") && !id.contains("$nuxt.$emit"))
      || context
        .template_state
        .vue_directives
        .iter()
        .any(|directive| directive.value.contains("$emit") && !directive.value.contains("$nuxt.$emit"))
  }

  /// Check for $emit in identifiers and function calls
  fn has_emit_in_identifiers(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .identifiers
      .iter()
      .any(|id| id.contains("$emit") && !id.contains("$nuxt.$emit"))
      || context
        .script_state
        .function_calls
        .iter()
        .any(|call| call.contains("$emit") && !call.contains("$nuxt.$emit"))
  }

  /// Check for $emit usage in method bodies
  fn has_emit_in_methods(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .method_details
      .iter()
      .any(|method| method.body.contains("$emit") && !method.body.contains("$nuxt.$emit"))
  }

  /// Check for $emit usage in computed property setters
  fn has_emit_in_computed(&self, context: &TransformationContext) -> bool {
    context
      .script_state
      .computed_details
      .iter()
      .any(|computed| {
        if let Some(setter) = &computed.setter {
          setter.contains("$emit") && !setter.contains("$nuxt.$emit")
        } else {
          false
        }
      })
  }

  /// Extract emit event names from method bodies and function calls
  fn extract_emit_events(&self, context: &TransformationContext) -> Vec<String> {
    let mut events = Vec::new();

    // Helper function to add unique events while preserving order
    let mut add_event = |event: String| {
      let mapped_event = self.map_event_name(&event);
      if !events.contains(&mapped_event) {
        events.push(mapped_event);
      }
    };

    // Check template function call details for $emit calls first (to get template events first)
    for call_detail in &context.template_state.function_call_details {
      if call_detail.name == "$emit" && !call_detail.arguments.is_empty() {
        // Find the first string literal argument (ignoring parentheses and other tokens)
        for arg in &call_detail.arguments {
          let cleaned_arg = arg.trim();
          // Skip parentheses and other tokens, look for quoted strings
          if (cleaned_arg.starts_with('\'') && cleaned_arg.ends_with('\'')) ||
             (cleaned_arg.starts_with('"') && cleaned_arg.ends_with('"')) ||
             (cleaned_arg.starts_with('`') && cleaned_arg.ends_with('`')) {
            let event_name = cleaned_arg.trim_matches('\'').trim_matches('"').trim_matches('`');
            if !event_name.is_empty() {
              add_event(event_name.to_string());
              break; // Only take the first string argument
            }
          }
        }
      }
    }

    // Check template directives for $emit calls
    for directive in &context.template_state.vue_directives {
      if let Some(extracted_events) = self.extract_events_from_body(&directive.value) {
        for event in extracted_events {
          add_event(event);
        }
      }
    }

    // Check method bodies for $emit calls
    for method in &context.script_state.method_details {
      if let Some(extracted_events) = self.extract_events_from_body(&method.body) {
        for event in extracted_events {
          add_event(event);
        }
      }
    }

    // Check computed property setters for $emit calls
    for computed in &context.script_state.computed_details {
      if let Some(setter) = &computed.setter {
        if let Some(extracted_events) = self.extract_events_from_body(setter) {
          for event in extracted_events {
            add_event(event);
          }
        }
      }
    }

    // Events are already mapped in add_event function
    events
  }

  /// Extract event names from a method body
  fn extract_events_from_body(&self, body: &str) -> Option<Vec<String>> {
    let mut events = Vec::new();
    
    // Look for Vue component $emit calls: this.$emit('eventName', ...)
    // Simple regex to find $emit calls, but exclude $nuxt.$emit patterns
    {
      let re = &*EMIT_TEMPLATE_PATTERN;
      for cap in re.captures_iter(body) {
        // Check if this $emit is part of $nuxt.$emit by looking at the text before the match
        let match_start = cap.get(0).unwrap().start();
        let text_before = if match_start >= 5 { &body[match_start-5..match_start] } else { &body[0..match_start] };
        
        if !text_before.ends_with("$nuxt.") {
          if let Some(event_name) = cap.get(1) {
            events.push(event_name.as_str().to_string());
          }
        }
      }
    }

    if events.is_empty() {
      None
    } else {
      Some(events)
    }
  }

  /// Map Vue2 event names to Vue3 equivalents
  fn map_event_name(&self, event: &str) -> String {
    match event {
      "input" => "update:value".to_string(),
      // Add more mappings as needed
      _ => event.to_string(),
    }
  }

  /// Generate the defineEmits setup code
  fn generate_emit_setup(&self, events: &[String]) -> String {
    if events.is_empty() {
      return String::new();
    }

    let events_list = events
      .iter()
      .map(|event| format!("'{}'", event))
      .collect::<Vec<_>>()
      .join(",");

    format!("const emit = defineEmits([{}]);", events_list)
  }
}

impl Transformer for EmitTransformer {
  fn name(&self) -> &'static str {
    "emit"
  }

  fn should_transform(&self, context: &TransformationContext, _config: &TransformerConfig) -> bool {
    self.has_emit_usage(context)
  }

  fn transform(
    &self,
    context: &TransformationContext,
    _config: &TransformerConfig,
  ) -> TransformationResult {
    let mut result = TransformationResult::default();

    if self.has_emit_usage(context) {
      let events = self.extract_emit_events(context);
      
      if !events.is_empty() {
        // Generate defineEmits setup code
        let emit_setup = self.generate_emit_setup(&events);
        result.add_setup(emit_setup);
        result.add_setup("".to_string()); // Add blank line
      }

      // Add template replacements for $emit -> emit
      result.template_replacements.push(crate::TemplateReplacement {
        find: "$emit(".to_string(),
        replace: "emit(".to_string(),
      });
    }

    result
  }

  fn get_body_transform(&self) -> Option<Box<BodyTransformFn>> {
    Some(Box::new(
      |body: &str, context: &TransformationContext, _config: &TransformerConfig| {
        let emit_transformer = EmitTransformer::new();
        let mut transformed_body = body.to_string();

        // Transform $emit usage
        if emit_transformer.has_emit_usage(context) {
          // Transform this.$emit calls to emit calls
          {
            let re = &*EMIT_THIS_PATTERN;
            transformed_body = re.replace_all(&transformed_body, |caps: &regex::Captures| {
              let event_name = &caps[1];
              let mapped_event = emit_transformer.map_event_name(event_name);
              format!("emit('{}'", mapped_event)
            }).to_string();
          }

          // Also handle cases without 'this.' (but exclude $nuxt.$emit)
          {
            let re = &*EMIT_TEMPLATE_PATTERN;
            transformed_body = re.replace_all(&transformed_body, |caps: &regex::Captures| {
              let full_match = caps.get(0).unwrap();
              let match_start = full_match.start();
              let text_before = if match_start >= 6 { &transformed_body[match_start-6..match_start] } else { &transformed_body[0..match_start] };
              
              if text_before.ends_with("$nuxt.") || text_before.ends_with("this.$nuxt.") {
                // This is a nuxt event bus emit, don't transform
                full_match.as_str().to_string()
              } else {
                let event_name = &caps[1];
                let mapped_event = emit_transformer.map_event_name(event_name);
                format!("emit('{}'", mapped_event)
              }
            }).to_string();
          }
        }

        transformed_body
      },
    ))
  }
}