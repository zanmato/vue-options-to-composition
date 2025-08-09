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
  fn test_should_handle_filters() {
    let sfc = r#"
<template>
  <h1>{{ thumbURL }}</h1>
</template>
<script>
export default {
  data() {
    return {
      product: {
        images: [
          { type: 'thumb', url: 'thumb1.png' },
          { type: 'full', url: 'full1.png' }
        ]
      }
    };
  },
  computed: {
    thumbURL() {
      return this.$options.filters.imagesByType(
        this.product.images,
        'thumb'
      );
    }
  }
}
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ thumbURL }}</h1>
</template>
<script setup>
import { computed, ref } from 'vue';
import { useFilters } from '@/composables/useFilters';

const { imagesByType } = useFilters();

const product = ref({
  images: [
    { type: 'thumb', url: 'thumb1.png' },
    { type: 'full', url: 'full1.png' }
  ]
});

const thumbURL = computed(() => {
  return imagesByType(product.value.images, 'thumb');
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_watchers() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
        <script>
        export default {
          data() {
            return {
              title: 'Hello world',
              count: 0
            };
          },
          watch: {
            count(newVal, oldVal) {
              console.log('Count changed from', oldVal, 'to', newVal);
              this.shout();
            },
            async userTrackingID(newID, oldID) {
              if (
                newID !== 'shiny'
              ) {
                return;
              }

              if (newID !== oldID && newID !== null) {
                await new Promise((resolve) => { resolve(123); });
              }
            },
          },
          methods: {
            shout() {
              console.log('Shouting:', this.title);
            }
          }
        }
        </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { ref, watch } from 'vue';

const count = ref(0);
const title = ref('Hello world');

watch(count, (newVal, oldVal) => {
  console.log('Count changed from', oldVal, 'to', newVal);
  shout();
});

watch(userTrackingID, async (newID, oldID) => {
  if (
  newID !== 'shiny'
  ) {
    return;
  }

  if (newID !== oldID && newID !== null) {
    await new Promise((resolve) => { resolve(123); });
  }
});

const shout = () => {
  console.log('Shouting:', title.value);
};
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_keep_script_usage_between_import_and_export() {
    let sfc = r#"<template>
<h1>Hello</h1>
</template>
<script>
import { Something } from './local.js';

const CookieName = '__consent';

const ConsentOption = Object.freeze({
  Necessary: 1,
  AdStorage: 1 << 1,
  AnalyticsStorage: 1 << 2,
  AdPersonalization: 1 << 3,
  AdUserData: 1 << 4,
});

export default {
  name: 'ConsentBanner',
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>Hello</h1>
</template>
<script setup>
import { Something } from './local.js';

const CookieName = '__consent';

const ConsentOption = Object.freeze({
  Necessary: 1,
  AdStorage: 1 << 1,
  AnalyticsStorage: 1 << 2,
  AdPersonalization: 1 << 3,
  AdUserData: 1 << 4,
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_alternative_data_declaration() {
    let sfc = r#"<template>
<h1>{{ scrollAmount }}</h1>
</template>
<script>
export default {
  data: () => ({
    scrollAmount: 0,
    catRow: {
      scrollWidth: 0,
      clientWidth: 0,
    },
  })
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ scrollAmount }}</h1>
</template>
<script setup>
import { ref } from 'vue';

const catRow = ref({
  scrollWidth: 0,
  clientWidth: 0,
});
const scrollAmount = ref(0);
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_set_and_delete() {
    let sfc = r#"<template>
<h1>Hej</h1>
</template>
<script>
export default {
  data() {
    return {
      filters: {}
    };
  },
  mounted() {
    const bob = 'color';

    this.$set(this.filters, 'normal', 'red');
    this.$set(this.filters, `f[${bob}]`, ['blue', 'green'].join(', '));
    this.$delete(this.filters, 'normal');
    this.$delete(this.filters, `f[${bob}]`);
  }
}
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>Hej</h1>
</template>
<script setup>
import { onMounted, ref } from 'vue';

const filters = ref({});

onMounted(() => {
  const bob = 'color';

  filters.value.normal = 'red';
  filters.value[`f[${bob}]`] = ['blue', 'green'].join(', ');
  delete filters.value.normal;
  delete filters.value[`f[${bob}]`];
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_all_lifecycle_hooks() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
      <script>
      export default {
        data() {
          return {
            title: 'Hello world'
          };
        },
        created() {
          console.log('Created');
        },
        mounted() {
          console.log('Mounted');
        },
        beforeUpdate() {
          console.log('Before Update');
        },
        updated() {
          console.log('Updated');
        },
        beforeUnmount() {
          console.log('Before Unmount');
        },
        unmounted() {
          console.log('Unmounted');
        },
        activated() {
          console.log('Activated');
        },
        deactivated() {
          console.log('Deactivated');
        },
        beforeDestroy() {
          console.log('Before Destroy');
        }
      }
      </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { onActivated, onBeforeUnmount, onBeforeUpdate, onDeactivated, onMounted, onUnmounted, onUpdated, ref } from 'vue';

const title = ref('Hello world');

console.log('Created');

onMounted(() => {
  console.log('Mounted');
});

onBeforeUpdate(() => {
  console.log('Before Update');
});

onUpdated(() => {
  console.log('Updated');
});

onBeforeUnmount(() => {
  console.log('Before Unmount');
  console.log('Before Destroy');
});

onUnmounted(() => {
  console.log('Unmounted');
});

onActivated(() => {
  console.log('Activated');
});

onDeactivated(() => {
  console.log('Deactivated');
});
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_this_in_methods() {
    let sfc = r#"<<template><h1 @click="$emit('send-it')">Hello</h1></template>
<script>
export default {
  props: {
    productSku: {
      type: String,
      default: () => null,
    },
    value: {
      type: Boolean,
      default: () => false,
    },
  },
  computed: {
    showModal: {
      get() {
        return this.value;
      },
      set(v) {
        this.$emit('input', v);
      }
    },
  },
  methods: {
    notify() {
      this.$axios
        .post('/api/product-bulk-order', {
          sku: this.productSku,
        });
    },
  },
};
</script>"#;

    let expected = r#"
<template>
<h1 @click="emit('send-it')">Hello</h1>
</template>
<script setup>
import { computed } from 'vue';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();

const props = defineProps({
  productSku: {
    type: String,
    default: () => null,
  },
  value: {
    type: Boolean,
    default: () => false,
  },
});

const emit = defineEmits(['send-it','update:value']);

const showModal = computed({
  get() {
    return props.value;
  },
  set(v) {
    emit('update:value', v);
  },
});

const notify = () => {
  http
  .post('/api/product-bulk-order', {
    sku: props.productSku,
  });
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_next_tick() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        this.$nextTick(() => {
          console.log('Next tick executed');
        });
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { nextTick, onMounted, ref } from 'vue';

const title = ref('Hello world');

onMounted(() => {
  nextTick(() => {
    console.log('Next tick executed');
  });
});
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_template_refs() {
    let sfc = r##"<template>
    <a href="#">anchor</a>
    <h1 ref="titleRef">{{ title }}</h1>
    <div ref="cat-row"></div>
    </template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        if (this.$refs?.titleRef) {
          console.log(this.$refs.titleRef);
        }

        console.log('Alternative syntax', this.$refs['cat-row']);
      }
    }
    </script>"##;

    let expected = r##"
<template>
  <a href="#">anchor</a>
  <h1 ref="titleRef">{{ title }}</h1>
  <div ref="cat-row"></div>
</template>
<script setup>
import { onMounted, ref, useTemplateRef } from 'vue';

const titleRef = useTemplateRef('titleRef');
const catRowRef = useTemplateRef('cat-row');
const title = ref('Hello world');

onMounted(() => {
  if (titleRef.value) {
    console.log(titleRef.value);
  }

  console.log('Alternative syntax', catRowRef.value);
});
</script>"##;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }
}
