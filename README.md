# Vue 2 to Vue 3 Migration Tool

This tool applies a best-effort of transform Vue 2 Options API to Vue 3 Composition API syntax. With a focus on Nuxt 2 migration.
The transformation won't be perfect, but it will get you very close.

The transformation assumes a few things,

- that you will provide a pinia store for each vuex store used (with the same name)
- that you will provide a composable `useFilters` for any Vue 2 filters used
- that you will provide a composable for each `mixin` used, see the configuration on how to provide that
- (nuxt2) that you will provide a `useNuxtCompat` composable for `asyncData`, `redirect`, `events` ($on, $off, $emit) and `refresh`
- (nuxt2) that you will provide a composable `useI18nUtils` for `localePath` and `localeProperties` usage
- (nuxt2) that you will use `@unhead/vue` for the `head()` functionality

**NOTE:** Indentation will be quite broken after the transformation, it's recommended to run some formatter on your code afterwards.

## Features

- 🔄 **Complete Vue 2 → Vue 3 transformation**

  - Options API to Composition API conversion
  - Data properties to `ref()` declarations
  - Methods to arrow functions
  - Computed properties with getter/setter support
  - Lifecycle hooks transformation
  - Watchers migration
  - Props and emits handling

- 📦 **Library and Framework Migration**

  - Vuex to Pinia store transformations
  - Custom mixins to composables conversion
  - Import path rewriting (e.g., bootstrap-vue → bootstrap-vue-next)
  - Component name transformations
  - Directive transformations

- 🔧 **Vue 2 API Compatibility**

  - `$set` and `$delete` → Vue 3 reactive assignments
  - `$refs` → `useTemplateRef()` composable
  - `$router`/`$route` → Vue Router composables
  - `$i18n` → Vue I18n composables
  - `$axios` → custom HTTP composables
  - Template transformations for directives and components

- 📁 **Flexible Processing**
  - Single file or directory processing
  - Recursive directory scanning
  - In-place transformation or output to different location
  - TOML configuration file support

## Installation

### From Source

Clone the repository and build with Cargo:

```bash
cargo build --release
```

The binary will be available at `target/release/vue-options-to-composition`.

### Using Cargo

```bash
cargo install vue-options-to-composition
```

## Usage

### Command Line Interface

```bash
vue-options-to-composition <input-path> [options]
```

#### Options

For complete usage information, run:

```bash
vue-options-to-composition --help
```

This will display:

```
Transform Vue 2 SFC to Vue 3 Composition API

Usage: vue-options-to-composition [OPTIONS] <input>

Arguments:
  <input>  Path to Vue SFC file or directory containing .vue files

Options:
  -c, --config <FILE>  Configuration TOML file path
  -o, --output <PATH>  Output file/directory path (default: overwrites input)
  -r, --recursive      Process directories recursively
  -h, --help           Print help
  -V, --version        Print version
```

#### Quick Examples

```bash
# Transform a single Vue component
vue-options-to-composition components/MyComponent.vue

# Transform all Vue files in a directory with configuration
vue-options-to-composition src/components/ -c config.toml

# Transform to a different output directory
vue-options-to-composition src/ -o dist/ -c migration-config.toml
```

## Configuration File

The migration tool uses a TOML configuration file to customize transformations. Create a `config.toml` file to define:

- **Import rewrites**: Transform import statements and component names
- **Vuex to Pinia**: Map Vuex modules to Pinia stores
- **Mixins to Composables**: Convert mixins to composition functions
- **Additional imports**: Handle auto-imported components

### Example Configuration

Create a `config.toml` file with your transformation settings. See `config.example.toml` for a complete example, or use this basic template:

```toml
# Vue Options to Composition API transformation configuration

[mixins.my_mixin]
name = "useMixin"
imports = ["mixinMethod1", "mixinMethod2"]

[imports_rewrite.bootstrap-vue]
name = "bootstrap-vue-next"

[imports_rewrite.bootstrap-vue.component_rewrite]
BSidebar = "BOffcanvas"
BNavbar = "BNavbar"

[imports_rewrite.bootstrap-vue.directives]
"v-b-toggle" = "vBToggle"

[vuex.user]
name = "user"
import_name = "useUserStore"

[vuex.cart]
name = "cart"
import_name = "useCartStore"

[additional_imports.ClientOnly]
import_path = "@/components/ClientOnly.vue"

[additional_imports.NuxtLink]
rewrite_to = "router-link"

import_keeplist = ["vue", "vue-router"]
```

### Configuration Schema

#### `imports_rewrite`

Rewrite import statements and transform component/directive names:

```toml
[imports_rewrite.bootstrap-vue]
name = "bootstrap-vue-next"

[imports_rewrite.bootstrap-vue.component_rewrite]
BSidebar = "BOffcanvas"

[imports_rewrite.bootstrap-vue.directives]
"v-b-toggle" = "vBToggle"
```

#### `vuex`

Map Vuex modules to Pinia stores:

```toml
[vuex.user]
name = "user"
import_name = "useUserStore"

[vuex.cart]
name = "cart"
import_name = "useCartStore"
```

#### `mixins`

Convert mixins to composables:

```toml
[mixins.price]
name = "usePrice"
imports = ["priceRaw", "priceRound", "currency"]
```

#### `additional_imports`

Handle additional component imports:

```toml
[additional_imports.ClientOnly]
import_path = "@/components/ClientOnly.vue"

[additional_imports.NuxtLink]
rewrite_to = "router-link"
```

## Supported Transformations

- ✅ Data properties → `ref()`
- ✅ Computed properties (get/set)
- ✅ Methods → Arrow functions
- ✅ Lifecycle hooks → Composition API hooks
- ✅ Watchers → `watch()`
- ✅ Props → `defineProps()`
- ✅ Emits → `defineEmits()`
- ✅ Vuex → Pinia stores
- ✅ Mixins → Composables
- ✅ `$refs` → `useTemplateRef()`
- ✅ `$router`/`$route` → Router composables
- ✅ `$i18n` → I18n composables
- ✅ `$set`/`$delete` → Native assignments
- ✅ Template transformations
- ✅ Import path rewriting
- ✅ Component name mapping

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request
