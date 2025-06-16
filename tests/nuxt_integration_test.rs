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
  fn test_should_handle_special_nuxt_object() {
    let sfc = r#"
<template>
  <h1>Hello</h1>
</template>
<script>
export default {
  name: "MyComponent",
  methods: {
    handleClick() {
      try {
        this.$nuxt.refresh();
      } catch (error) {
        this.$nuxt.context.redirect(this.localePath("home"));
      }
    }
  },
  mounted() {
    this.$nuxt.$emit("custom-event", { data: "test" });
  }
}
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>Hello</h1>
</template>
<script setup>
import { onMounted } from 'vue';
import { useI18nUtils } from '@/composables/useI18nUtils';
import { useNuxtCompat } from '@/composables/useNuxtCompat';

const { eventBus, redirect, refresh } = useNuxtCompat();
const { localePath } = useI18nUtils();

const handleClick = () => {
  try {
    refresh();
  } catch (error) {
    redirect(localePath("home"));
  }
};

onMounted(() => {
  eventBus.emit("custom-event", { data: "test" });
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_legacy_async_data_method() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: '',
          links: []
        };
      },
      async asyncData({ $axios, app, redirect, params }) {
        const data = await $axios.get('https://api.example.com/data');

        const links = ['nightowl'];
        return {
          title: data.title,
          links
        };
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';
import { useAsyncData } from '@/composables/useAsyncData';

const data = await useAsyncData(async ({ $axios, app, redirect, params }) => {
  const data = await $axios.get('https://api.example.com/data');

  const links = ['nightowl'];
  return {
    title: data.title,
    links
  };
});

const links = ref(data.links);
const title = ref(data.title);
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_nuxt_i18n_paths() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      nuxtI18n: {
        paths: {
          no: '/produkt/:slug',
          sv: '/produkt/:slug',
          fi: '/tuote/:slug',
          da: '/produkt/:slug',
          nl: '/product/:slug',
        },
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';

const title = ref('Hello world');
</script>
<script>
export const i18n = {
  no: '/produkt/:slug',
  sv: '/produkt/:slug',
  fi: '/tuote/:slug',
  da: '/produkt/:slug',
  nl: '/product/:slug',
};
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_nuxt_event_bus() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        this.$nuxt.$on('custom-event', this.handleCustomEvent);
      },
      beforeDestroy() {
        this.$nuxt.$off('custom-event', this.handleCustomEvent);
      },
      methods: {
        handleCustomEvent(data) {
          console.log('Custom event received:', data);

          this.$nuxt.$emit('another-event', { message: 'Hello from custom event' });
        }
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useNuxtCompat } from '@/composables/useNuxtCompat';

const { eventBus } = useNuxtCompat();

const title = ref('Hello world');

const handleCustomEvent = (data) => {
  console.log('Custom event received:', data);

  eventBus.emit('another-event', { message: 'Hello from custom event' });
};

onMounted(() => {
  eventBus.on('custom-event', handleCustomEvent);
});

onBeforeUnmount(() => {
  eventBus.off('custom-event', handleCustomEvent);
});
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_config() {
    let sfc = r#"<template><h1 :title="$config[$i18n.locale].appName">{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: this.$config[this.$i18n.locale].appName
        };
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1 :title="runtimeConfig[locale].appName">{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useNuxtCompat } from '@/composables/useNuxtCompat';

const { runtimeConfig } = useNuxtCompat();
const { locale } = useI18n();

const title = ref(runtimeConfig[locale.value].appName);
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }
}
