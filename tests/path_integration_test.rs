use vue_options_to_composition::rewrite_sfc;

fn trim_whitespace(s: &str) -> String {
  s.lines()
    .map(|line| line.trim())
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn test_should_rewrite_img_paths() {
    let sfc = r#"
<template>
  <img src="~/assets/logo.png" alt="Logo" />
</template>
<script>
export default {
  name: "MyComponent"
}
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <img src="@/assets/logo.png" alt="Logo" />
</template>
<script setup>
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }
}
