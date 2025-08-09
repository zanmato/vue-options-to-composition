use std::collections::HashMap;
use vue_options_to_composition::{
  rewrite_sfc, AdditionalImport, ImportRewrite, MixinConfig, RewriteOptions,
};

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
  fn test_should_convert_data_to_refs() {
    let sfc = r#"<template><h1>{{ count }}</h1></template>
    <script>
    export default {
      data() {
        return {
          count: 0,
          countOptions: [],
        };
      },
      methods: {
        increment() {
          this.count++;
          console.log(this.countOptions);
        }
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ count }}</h1>
</template>
<script setup>
import { ref } from 'vue';

const count = ref(0);
const countOptions = ref([]);

const increment = () => {
  count.value++;
  console.log(countOptions.value);
};
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_i18n_methods() {
    let sfc = r#"<template>
    <h1>{{ $t('hello') }}</h1>
    <span class="sum">
      {{
        $n(count, {
          key: 'currency',
        })
      }}
    </span>
    <span :title="$t('hello')">{{ $d(Date.now(), 'short') }}</span>
    </template>
    <script>
    export default {
      data() {
        return {
          count: 0,
          somethingTranslated: this.$t('hello')
        };
      },
      head() {
        return {
          title: this.$t('page.title')
        };
      },
      methods: {
        greet() {
          return this.$t('hello');
        }
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ t('hello') }}</h1>
  <span class="sum">
    {{
      n(count, {
        key: 'currency',
      })
    }}
  </span>
  <span :title="t('hello')">{{ d(Date.now(), 'short') }}</span>
</template>
<script setup>
import { ref } from 'vue';
import { useHead } from '@unhead/vue';
import { useI18n } from 'vue-i18n';

const { t, n, d } = useI18n();

const count = ref(0);
const somethingTranslated = ref(t('hello'));

const greet = () => {
  return t('hello');
};

useHead(() => {
  return {
    title: t('page.title'),
  };
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_i18n_template_usage() {
    let sfc = r#"<template>
    <h1>{{ $t('hello') }}</h1>
    <span>{{
      $t('estimation', {
        estimated_at: $d(new Date(), 'short'),
      })
    }}</span>
    </template>
    <script>
    export default {}
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ t('hello') }}</h1>
  <span>{{
    t('estimation', {
      estimated_at: d(new Date(), 'short'),
    })
  }}</span>
</template>
<script setup>
import { useI18n } from 'vue-i18n';

const { t, d } = useI18n();
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_nuxt_fetch() {
    let sfc = r#"<template><h1 @click="clickHandler">{{ data }}</h1></template>
    <script>
    export default {
      async fetch() {
        const res = await this.$axios.get('https://api.example.com/data');

        this.data = res.data;
        this.rows = res.headers['x-total-count'];
      },
      data() {
        return {
          data: null,
          rows: 0
        };
      },
      methods: {
        clickHandler() {
          this.$fetch();
        }
      },
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1 @click="clickHandler">{{ data }}</h1>
</template>
<script setup>
import { onMounted, ref } from 'vue';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();

const data = ref(null);
const rows = ref(0);

const fetch = async () => {
  const res = await http.get('https://api.example.com/data');

  data.value = res.data;
  rows.value = res.headers['x-total-count'];
};

const clickHandler = () => {
  fetch();
};

onMounted(async () => {
  fetch();
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_custom_mixins() {
    let sfc = r#"<template><h1>{{ title }}{{ priceRaw(100) }}</h1></template>
    <script>
    import pp from 'ninth-dimension';
    import priceMixin from '@/mixins/price';

    export default {
      mixins: [priceMixin],
      data() {
        return {
          title: 'Hello World',
          yolo: []
        };
      },
      computed: {
        cartSum() {
          return this.yolo.reduce(
            (p, v) =>
              p +
              this.priceRaw(
                v.price,
              ) *
                v.quantity,
            0
          );
        }
      },
      methods: {
        featureDetection() {
          const paymentRequest = pp.paymentRequest({
            currency: this.currency.toLowerCase(),
            total: {
              label: 'Total',
              amount: price(100, 2),
            },
          });
        },
        claw() {
          this.price(
            100,
            10
          );
        }
      }
    }
    </script>"#;

    let mut mixins = HashMap::new();
    mixins.insert(
      "price".to_string(),
      MixinConfig {
        name: "usePrice".to_string(),
        imports: vec![
          "currency".to_string(),
          "maximumFractionDigits".to_string(),
          "priceRaw".to_string(),
          "priceDiscountRaw".to_string(),
          "filterPrices".to_string(),
          "lowestPrice".to_string(),
          "fromPrice".to_string(),
          "price".to_string(),
          "discountPrice".to_string(),
          "priceRound".to_string(),
        ],
      },
    );

    let options = RewriteOptions {
      mixins: Some(mixins),
      ..Default::default()
    };

    let result = rewrite_sfc(sfc, Some(options)).unwrap();

    let expected = r#"
<template>
  <h1>{{ title }}{{ priceRaw(100) }}</h1>
</template>
<script setup>
import { computed, ref } from 'vue';
import pp from 'ninth-dimension';
import { usePrice } from '@/composables/usePrice';

const { priceRaw, currency, price } = usePrice();

const title = ref('Hello World');
const yolo = ref([]);

const cartSum = computed(() => {
  return yolo.value.reduce(
    (p, v) =>
      p +
      priceRaw(
        v.price,
      ) *
      v.quantity,
      0
    );
});

const featureDetection = () => {
  const paymentRequest = pp.paymentRequest({
    currency: currency.toLowerCase(),
    total: {
      label: 'Total',
      amount: price(100, 2),
    },
  });
};

const claw = () => {
  price(
    100,
    10
  );
};
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_import_rewrites() {
    let sfc = r#"
<template>
  <ClientOnly>
    <h1 v-b-toggle>{{ title }}</h1>
  </ClientOnly>
  <nuxt-link :to="{ name: 'index' }">link</nuxt-link>
  <b-button></b-button>
  <BButton></BButton>
  <BSidebar></BSidebar>
  <b-sidebar></b-sidebar>
</template>
<script>
import { BSidebar, BButton } from 'bootstrap-vue';

export default {
  components: {
    BSidebar,
    BButton
  },
  data() {
    return {
      title: 'Hello Bootstrap Vue'
    };
  }
}
</script>"#;

    let mut component_rewrite = HashMap::new();
    component_rewrite.insert("BSidebar".to_string(), "BOffcanvas".to_string());

    let mut directives = HashMap::new();
    directives.insert("v-b-toggle".to_string(), "vBToggle".to_string());

    let mut imports_rewrite = HashMap::new();
    imports_rewrite.insert(
      "bootstrap-vue".to_string(),
      ImportRewrite {
        name: "bootstrap-vue-next".to_string(),
        component_rewrite: Some(component_rewrite),
        directives: Some(directives),
      },
    );

    let mut additional_imports = HashMap::new();
    additional_imports.insert(
      "ClientOnly".to_string(),
      AdditionalImport {
        import_path: Some("import ClientOnly from '@/components/ClientOnly.vue';".to_string()),
        rewrite_to: None,
      },
    );
    additional_imports.insert(
      "NuxtLink".to_string(),
      AdditionalImport {
        import_path: None,
        rewrite_to: Some("router-link".to_string()),
      },
    );

    let options = RewriteOptions {
      imports_rewrite: Some(imports_rewrite),
      additional_imports: Some(additional_imports),
      ..Default::default()
    };

    let result = rewrite_sfc(sfc, Some(options)).unwrap();

    let expected = r#"
<template>
  <ClientOnly>
    <h1 v-b-toggle>{{ title }}</h1>
  </ClientOnly>
  <router-link :to="{ name: 'index' }">link</router-link>
  <b-button></b-button>
  <BButton></BButton>
  <BOffcanvas></BOffcanvas>
  <b-offcanvas></b-offcanvas>
</template>
<script setup>
import { ref } from 'vue';
import { BButton, BOffcanvas, vBToggle } from 'bootstrap-vue-next';
import ClientOnly from '@/components/ClientOnly.vue';

const title = ref('Hello Bootstrap Vue');
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_props() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      props: {
        title: {
          type: String,
          required: true
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
const props = defineProps({
  title: {
    type: String,
    required: true,
  },
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_computed_properties() {
    let sfc = r#"<template><h1>{{ fullName }}</h1></template>
    <script>
    export default {
      data() {
        return {
          firstName: 'John',
          lastName: 'Doe'
        };
      },
      computed: {
        fullName: {
          get() {
            return `${this.firstName} ${this.lastName}`;
          },
          set(v) {
            const names = v.split(' ');
            this.firstName = names[0];
            this.lastName = names[1];
          }
        }
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ fullName }}</h1>
</template>
<script setup>
import { computed, ref } from 'vue';

const firstName = ref('John');
const lastName = ref('Doe');

const fullName = computed({
  get() {
    return `${firstName.value} ${lastName.value}`;
  },
  set(v) {
    const names = v.split(' ');
    firstName.value = names[0];
    lastName.value = names[1];
  },
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_keep_computed_setter_name() {
    let sfc = r#"<template><h1>{{ fullName }}</h1></template>
    <script>
    export default {
      data() {
        return {
          firstName: 'John',
          lastName: 'Doe'
        };
      },
      computed: {
        fullName: {
          get() {
            return `${this.firstName} ${this.lastName}`;
          },
          set(value) {
            const names = value.split(' ');
            this.firstName = names[0];
            this.lastName = names[1];
          }
        }
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ fullName }}</h1>
</template>
<script setup>
import { computed, ref } from 'vue';

const firstName = ref('John');
const lastName = ref('Doe');

const fullName = computed({
  get() {
    return `${firstName.value} ${lastName.value}`;
  },
  set(value) {
    const names = value.split(' ');
    firstName.value = names[0];
    lastName.value = names[1];
  },
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_head_method() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello World'
        };
      },
      head() {
        return {
          title: this.title
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
import { useHead } from '@unhead/vue';

const title = ref('Hello World');

useHead(() => {
  return {
    title: title.value,
  };
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_complex_head_method() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello World'
        };
      },
      head() {
        const head = {
          title: this.title,
          meta: [
            { name: 'description', content: 'This is a description' },
            { property: 'og:title', content: this.title }
          ]
        };

        return head;
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
import { useHead } from '@unhead/vue';

const title = ref('Hello World');

useHead(() => {
  const head = {
    title: title.value,
    meta: [
      { name: 'description', content: 'This is a description' },
      { property: 'og:title', content: title.value }
    ]
  };

  return head;
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_simple_components() {
    let sfc = r#"<template><h1>Dummy content</h1></template>
    <script>
    export default {
      name: 'DummyComponent',
    };
    </script>
    <style scoped>
    h1 {
      color: red;
    }
    </style>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"<template>
    <h1>Dummy content</h1>
</template>
<script setup>
</script>
<style scoped>
h1 {
  color: red;
}
</style>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_localeprops() {
    let sfc = r#"<template>
    <span>{{ $i18n.localeProperties.brand }}</span>
    </template>
    <script>
    export default {
    }
    </script>"#;

    let expected = r#"
<template>
  <span>{{ localeProperties.brand }}</span>
</template>
<script setup>
import { useI18nUtils } from '@/composables/useI18nUtils';

const { localeProperties } = useI18nUtils();
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_i18n_utils() {
    let sfc = r#"<template><nuxt-link :to="localePath('my-account')">{{ title }}</nuxt-link>
    <span>{{ $i18n.locale }}</span>
    <span>{{ $i18n.localeProperties.brand }}</span>
    </template>
    <script>
    export default {
      data() {
        return {
          title: this.$i18n.localeProperties.brand
        };
      },
      head() {
        return {
          title: this.$i18n.localeProperties.brand,
          htmlAttrs: {
            lang: this.$i18n.locale,
          }
        };
      },
      mounted() {
        console.log('Great locale, tremendous', this.$i18n.locale);
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <router-link :to="localePath('my-account')">{{ title }}</router-link>
  <span>{{ locale }}</span>
  <span>{{ localeProperties.brand }}</span>
</template>
<script setup>
import { onMounted, ref } from 'vue';
import { useHead } from '@unhead/vue';
import { useI18n } from 'vue-i18n';
import { useI18nUtils } from '@/composables/useI18nUtils';

const { locale } = useI18n();
const { localeProperties, localePath } = useI18nUtils();

const title = ref(localeProperties.brand);

useHead(() => {
  return {
    title: localeProperties.brand,
    htmlAttrs: {
      lang: locale.value,
    }
  };
});

onMounted(() => {
  console.log('Great locale, tremendous', locale.value);
});
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_keep_existing_imports() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    import vSelect from 'vue-select';
    import MyComponent from '@/components/MyComponent.vue';
    import AnotherComponent from '~/components/AnotherComponent.vue';

    const BigAsyncComponent = () => import('@/components/BigAsyncComponent.vue');
    const AnotherBigAsyncComponent = () => import('~/components/BigAsyncComponent.vue');

    export default {
      components: {
        MyComponent,
        AnotherComponent,
        BigAsyncComponent,
        AnotherBigAsyncComponent,
      },
      data() {
        return {
          title: 'Hello world'
        };
      }
    }
    </script>>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { defineAsyncComponent, ref } from 'vue';
import vSelect from 'vue-select';
import MyComponent from '@/components/MyComponent.vue';
import AnotherComponent from '@/components/AnotherComponent.vue';
const BigAsyncComponent = defineAsyncComponent(() => import('@/components/BigAsyncComponent.vue'));
const AnotherBigAsyncComponent = defineAsyncComponent(() => import('@/components/BigAsyncComponent.vue'));

const title = ref('Hello world');
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_route_and_router() {
    let sfc = r#"<template>
    <h1>{{ title }}</h1>
    <span>{{ $route.params.id }}</span>
    </template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        console.log(this.$route.path);
        this.$router.push('/new-path');
      }
    }
    </script>"#;

    let expected = r##"
<template>
  <h1>{{ title }}</h1>
  <span>{{ route.params.id }}</span>
</template>
<script setup>
import { onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';

const route = useRoute();
const router = useRouter();

const title = ref('Hello world');

onMounted(() => {
  console.log(route.path);
  router.push('/new-path');
});
</script>"##;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_event_listeners() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        window.addEventListener('resize', this.handleResize);
      },
      beforeDestroy() {
        window.removeEventListener('resize', this.handleResize);
      },
      methods: {
        handleResize() {
          console.log('Window resized');
        }
      }
    }
    </script>"#;

    let expected = r##"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';

const title = ref('Hello world');

const handleResize = () => {
  console.log('Window resized');
};

onMounted(() => {
  window.addEventListener('resize', handleResize);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
});
</script>"##;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_async_methods() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      methods: {
        async fetchData() {
          const res = await this.$axios.get('https://api.example.com/data');
          this.title = res.data.title;
        }
      },
      mounted() {
        this.fetchData();
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { onMounted, ref } from 'vue';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();

const title = ref('Hello world');

const fetchData = async () => {
  const res = await http.get('https://api.example.com/data');
  title.value = res.data.title;
};

onMounted(() => {
  fetchData();
});
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_add_fixme_if_variable_doesnt_exist() {
    let sfc = r#"<template>
    <h1>{{ title }}</h1>
  </template>
  <script>
    export default {
      data() {
        return {
          title: 'Hello world',
          count: 0
        };
      },
      methods: {
        increment() {
          this.count++;
          console.log(this.nonExistentVariable);
        }
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';

const count = ref(0);
const title = ref('Hello world');

const increment = () => {
  count.value++;
  console.log(/* FIXME: nonExistentVariable */ nonExistentVariable);
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_spread_operator_with_this() {
    let sfc = r#"<template><h1 @click="handleClick">{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world',
          messageSent: false,
          sending: false,
          errorSending: false,
          form: {
            name: '',
            email: ''
          }
        };
      },
      methods: {
        handleClick() {
          this.sending = true;
          this.$axios
            .post('/api/form', { ...this.form })
            .then(() => {
              this.messageSent = true;
            })
            .catch(() => {
              this.errorSending = true;
            })
            .finally(() => {
              this.sending = false;
            });
        }
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1 @click="handleClick">{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();

const errorSending = ref(false);
const form = ref({
  name: '',
  email: ''
});
const messageSent = ref(false);
const sending = ref(false);
const title = ref('Hello world');

const handleClick = () => {
  sending.value = true;
  http
    .post('/api/form', { ...form.value })
    .then(() => {
      messageSent.value = true;
    })
    .catch(() => {
      errorSending.value = true;
    })
    .finally(() => {
      sending.value = false;
    });
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_regex_data() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world',
          regex: /\\d+/g
        };
      },
      methods: {
        testRegex() {
          return this.regex.test('123');
        }
      }
    }
    </script>"#;

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { ref } from 'vue';

const regex = ref(/\\d+/g);
const title = ref('Hello world');

const testRegex = () => {
  return regex.value.test('123');
};
</script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }
}
