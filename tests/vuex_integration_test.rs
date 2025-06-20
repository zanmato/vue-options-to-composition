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
  fn test_should_handle_direct_vuex_usage() {
    let sfc = r#"<template><h1>{{ title }}</h1></template>
    <script>
    export default {
      data() {
        return {
          title: 'Hello world'
        };
      },
      mounted() {
        this.$store.commit('user/updateUser', { name: 'New User' });
        this.$store.dispatch('user/fetchUser');
        this.$store.state.cart.items = [];
      }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ title }}</h1>
</template>
<script setup>
import { onMounted, ref } from 'vue';
import { useCartStore } from '@/stores/cart';
import { useUserStore } from '@/stores/user';

const cartStore = useCartStore();
const userStore = useUserStore();

const title = ref('Hello world');

onMounted(() => {
  userStore.updateUser({ name: 'New User' });
  userStore.fetchUser();
  cartStore.items = [];
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_vuex_usage() {
    let sfc = r#"<template>
        <h1>{{ user.name }}{{ userID }}{{ $store.state.user.userID }}</h1>
        <span v-if="$store.state.user.userID === '123'" :title="$store.state.user.userID">
          User is 123
        </span>
      </template>
      <script>
      import { mapState, mapGetters, mapActions, mapMutations } from 'vuex';

      export default {
        computed: {
          ...mapGetters({
            user: 'user/getUser',
            hasGrants: 'cart/hasGrants',
          }),
          ...mapState('user', {
            userID: 'userID'
          })
        },
        methods: {
          someMethod() {
            if (
              this.$store.state.user.userID === '123'
            ) {
              console.log('YES!');
            }
          },
          async postSomething() {
            try {
              await this.$axios.post('https://api.example.com/data', {
                userID: this.$store.state.user.userID
              });
            } catch (error) {
              console.error('Error posting data:', error);
            }
          },
          ...mapActions({ fetchUser: 'user/fetchUser', checkoutEvent: 'cart/checkoutEvent' }),
          ...mapMutations({ updateUser: 'user/updateUser' })
        },
        mounted() {
          this.fetchUser();
          this.checkoutEvent();

          console.log('Crazy user', this.$store.state.user.userID);
        }
      }
      </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ user.name }}{{ userID }}{{ userStore.userID }}</h1>
  <span v-if="userStore.userID === '123'" :title="userStore.userID">
    User is 123
  </span>
</template>
<script setup>
import { computed, onMounted } from 'vue';
import { useCartStore } from '@/stores/cart';
import { useUserStore } from '@/stores/user';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();
const cartStore = useCartStore();
const userStore = useUserStore();

const user = computed(() => userStore.getUser);
const userID = computed(() => userStore.userID);

const someMethod = () => {
  if (
    userStore.userID === '123'
  ) {
    console.log('YES!');
  }
};

const postSomething = async () => {
  try {
    await http.post('https://api.example.com/data', {
      userID: userStore.userID
    });
  } catch (error) {
    console.error('Error posting data:', error);
  }
};

onMounted(() => {
  userStore.fetchUser();
  cartStore.checkoutEvent();

  console.log('Crazy user', userStore.userID);
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_alternative_vuex_map_syntax() {
    let sfc = r#"<template><h1>{{ user.name }}</h1></template>
      <script>
      import { mapState, mapGetters, mapActions, mapMutations } from 'vuex';

      export default {
        computed: {
          ...mapState('user', ['userID']),
          ...mapGetters('user', ['getUser'])
        },
        methods: {
          ...mapActions('user', ['fetchUser']),
          ...mapMutations('user', ['updateUser'])
        },
        mounted() {
          console.log(this.userID);
          this.fetchUser();
          this.updateUser();
          console.log('User updated');
          console.log('User ID:', this.userID);
        }
      }
      </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
  <h1>{{ user.name }}</h1>
</template>
<script setup>
import { computed, onMounted } from 'vue';
import { useUserStore } from '@/stores/user';

const userStore = useUserStore();

const user = computed(() => userStore.getUser());
const userID = computed(() => userStore.userID);

onMounted(() => {
  console.log(userID.value);
  userStore.fetchUser();
  userStore.updateUser();
  console.log('User updated');
  console.log('User ID:', userID.value);
});
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }

  #[test]
  fn test_should_handle_imports_for_direct_vuex_usage() {
    let sfc = r#"<template><h1>Hello</h1></template>
    <script>
    export default {
      methods: {
        async fetchData() {
          try {
            const response = await this.$axios.get('https://api.example.com/data', {
              params: { userID: this.$store.state.user.userID }
            });
            console.log('Data fetched:', response.data);
          } catch (error) {
            console.error('Error fetching data:', error);
          }
        }
    }
    }
    </script>"#;

    let result = rewrite_sfc(sfc, None).unwrap();

    let expected = r#"
<template>
<h1>Hello</h1>
</template>
<script setup>
import { useUserStore } from '@/stores/user';
import { useHttp } from '@/composables/useHttp';

const http = useHttp();
const userStore = useUserStore();

const fetchData = async () => {
  try {
    const response = await http.get('https://api.example.com/data', {
      params: { userID: userStore.userID }
    });
    console.log('Data fetched:', response.data);
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};
</script>"#;

    assert_eq!(trim_whitespace(&result), trim_whitespace(expected));
  }
}
