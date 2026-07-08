---
title: Writing Plugins for TerminusDB
nextjs:
  metadata:
    title: Writing Plugins for TerminusDB
    description: Learn how to write custom plugins for TerminusDB to extend database functionality with hooks and custom behavior.
    keywords: terminusdb, writing plugins for terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/writing-plugins/
media: []
tags:
  - how-to
  - advanced
  - self-hosted
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally
- Familiarity with Prolog or the TerminusDB extension API
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to write and register plugins for TerminusDB.
{% /callout %}

## Writing Plugins for TerminusDB

TerminusDB supports a plugin system that allows you to extend the database with custom functionality through hooks that execute at specific points in the database lifecycle. Plugins are written in Prolog and can intercept operations like commits, queries, and schema changes.

### Plugin Architecture Overview

Plugins in TerminusDB:
- Are written in Prolog (`.pl` files)
- Are loaded at runtime from the `/plugins` directory
- Use the `plugins` multifile predicates to hook into database events
- Run in the context of the compiled TerminusDB binary

### Important: Module Loading for Plugins

**Critical consideration when writing plugins:** TerminusDB compiles into a monolithic binary that includes all core modules. When your plugin loads at runtime, it must reference modules correctly to avoid import errors.

#### Correct: Use Parent Modules

Always import from **parent modules** that are exposed by the binary:

```prolog
:- module('plugins/my_plugin', []).
:- use_module(core(api)).          % Parent module
:- use_module(core(util)).         % Parent module  
:- use_module(core(query)).        % Parent module
:- use_module(library(lists)).     % Standard library
```

#### Incorrect: Avoid Submodule Paths

Do **not** import submodules directly - they may not be accessible:

```prolog
:- module('plugins/my_plugin', []).
:- use_module(core(util/json_log)).     % Submodule - will fail!
:- use_module(core(api/api_error)).     % Submodule - will fail!
:- use_module(core(query/woql_compile)). % Submodule - will fail!
```

**Why?** The monolithic binary compiles modules hierarchically. Parent modules like `core(util)` are exported and re-export their submodule predicates, but the submodule paths themselves (`core(util/json_log)`) are not directly accessible to runtime-loaded code.

**Error you'll see if you do this wrong:**
```
ERROR: source_sink `core(util/json_log)' does not exist
```

### Available Plugin Hooks

#### Post-Commit Hook

The most common plugin hook executes after a successful commit:

```prolog
:- multifile plugins:post_commit_hook/2.

plugins:post_commit_hook(Validation_Objects, Meta_Data) :-
    % Your custom logic here
    % Validation_Objects contains the committed data
    % Meta_Data contains commit metadata
    true.
```

**Use cases:**
- Triggering optimization after data changes
- Sending notifications
- Updating external indexes
- Logging commit activity
- Triggering downstream processes

### Example Plugin: Auto-Optimizer

Here's a real-world example from TerminusDB's built-in auto-optimize plugin:

```prolog
:- module('plugins/auto_optimize', []).

% Import parent modules - these are accessible from the binary
:- use_module(core(api)).
:- use_module(core(util)).  % Includes json_log exports
:- use_module(core(query/resolve_query_resource)).
:- use_module(library(http/http_server)).
:- use_module(library(random)).
:- use_module(library(lists)).
:- use_module(library(thread_pool)).

% Define optimization probability per descriptor type
optimize_chance(system_descriptor{}, C) =>
    C = 0.1.
optimize_chance(D, C), database_descriptor{} :< D =>
    C = 0.1.
optimize_chance(D, C), repository_descriptor{} :< D =>
    C = 0.1.
optimize_chance(D, C), branch_descriptor{} :< D =>
    C = 0.1.
optimize_chance(_, _) => false.

% Decide whether to optimize based on probability
should_optimize(Descriptor) :-
    optimize_chance(Descriptor, Chance),
    random(X),
    X < Chance.

% Execute optimization and log result
optimize(Descriptor) :-
    api_optimize:descriptor_optimize(Descriptor),
    resolve_absolute_string_descriptor(Path, Descriptor),
    json_log_debug_formatted("Optimized ~s", [Path]).

% Get all parent descriptors
all_descriptor(Descriptor, Descriptor).
all_descriptor(Descriptor, Parent_Descriptor) :-
    get_dict(repository_descriptor, Descriptor, Intermediate_Descriptor),
    all_descriptor(Intermediate_Descriptor, Parent_Descriptor).
all_descriptor(Descriptor, Parent_Descriptor) :-
    get_dict(database_descriptor, Descriptor, Intermediate_Descriptor),
    all_descriptor(Intermediate_Descriptor, Parent_Descriptor).

% Process all validation objects for optimization
optimize_all(Validation_Objects) :-
    forall((member(V, Validation_Objects),
            all_descriptor(V.descriptor, Descriptor),
            should_optimize(Descriptor)),
           optimize(Descriptor)).

% Create a thread pool for optimization tasks
:- multifile thread_pool:create_pool/1.
thread_pool:create_pool(terminusdb_optimizer) :-
    current_prolog_flag(cpu_count, Count),
    thread_pool_create(terminusdb_optimizer, Count, []).

% Hook into post-commit events
plugins:post_commit_hook(Validation_Objects, _Meta_Data) :-
    (   http_server_property(_, _)
    ->  catch(thread_create_in_pool(terminusdb_optimizer,
                              optimize_all(Validation_Objects),
                              _,
                              [wait(false)]),
              error(resource_error(threads_in_pool(terminusdb_optimizer)), _),
              true
        )
    ;   optimize_all(Validation_Objects)
    ).
```

### Key Features Demonstrated

1. **Probabilistic execution**: Only runs on 10% of commits to avoid overhead
2. **Asynchronous processing**: Uses thread pools to avoid blocking commits
3. **Error handling**: Gracefully handles thread pool exhaustion
4. **Logging integration**: Uses `json_log_debug_formatted/2` from `core(util)`
5. **Descriptor traversal**: Walks the database hierarchy to find optimization candidates

### Available Predicates from Core Modules

When you import `core(util)`, you get access to:

**Logging:**
- `json_log_error/1`, `json_log_error/3`, `json_log_error_formatted/2`
- `json_log_warning/1`, `json_log_warning/3`, `json_log_warning_formatted/2`
- `json_log_notice/1`, `json_log_notice/3`, `json_log_notice_formatted/2`
- `json_log_info/1`, `json_log_info/3`, `json_log_info_formatted/2`
- `json_log_debug/1`, `json_log_debug/3`, `json_log_debug_formatted/2`

**File operations:**
- `terminus_path/1`, `touch/1`, `sanitise_file_name/2`
- `subdirectories/2`, `files/2`, `directories/2`

**Type checking:**
- `is_literal/1`, `is_uri/1`, `is_id/1`, `is_bnode/1`
- `is_graph_identifier/1`, `is_database_identifier/1`

When you import `core(api)`, you get access to:

**Database operations:**
- `api_optimize:descriptor_optimize/1`
- `api_document:insert_document/4`, `api_document:replace_document/4`
- `api_graph:create_graph/2`, `api_graph:delete_graph/2`

**Query operations:**
- `api_query:run_query/5`

### Plugin Deployment

#### Development

Place your plugin file in `src/plugins/`:

```bash
src/plugins/my_plugin.pl
```

Restart TerminusDB to load the plugin:

```bash
./terminusdb serve
```

#### Docker

For Docker deployments, mount your plugin into `/plugins`:

```bash
docker run -v /path/to/my_plugin.pl:/plugins/my_plugin.pl \
  terminusdb/terminusdb-server:latest
```

Or build a custom Docker image:

```dockerfile
FROM terminusdb/terminusdb-server:latest
COPY my_plugin.pl /plugins/
```

### Testing Your Plugin

1. **Enable debug logging** to see plugin activity:

```bash
export TERMINUSDB_LOG_LEVEL=debug
./terminusdb serve
```

2. **Test with a commit operation**:

```bash
# Using the CLI
./terminusdb doc insert admin/tdb-example-mydb '{"@type": "Person", "name": "Alice"}'

# Or using a client
```

3. **Check logs** for plugin execution:

```bash
# Look for your plugin's log messages
tail -f /var/log/terminusdb.log
```

### Best Practices

1. **Keep plugins lightweight**: Post-commit hooks run after every commit
2. **Use asynchronous execution**: Don't block the commit process
3. **Handle errors gracefully**: Use `catch/3` to prevent plugin failures from breaking commits
4. **Use probabilistic execution**: For expensive operations, only run on a percentage of commits
5. **Log appropriately**: Use `json_log_debug/2` for development, `json_log_info/2` for production events
6. **Test thoroughly**: Plugin errors can break database operations

### Debugging Plugin Load Errors

If you see module loading errors:

```
ERROR: source_sink `core(util/json_log)' does not exist
```

**Fix:** Change from submodule to parent module:

```prolog
% Change this:
:- use_module(core(util/json_log)).

% To this:
:- use_module(core(util)).
```

### Advanced: Custom Multifile Predicates

You can define your own multifile predicates for other plugins to extend:

```prolog
% In your plugin
:- multifile my_plugin:custom_hook/2.

my_plugin:trigger_custom_hook(Data, Context) :-
    forall(my_plugin:custom_hook(Data, Context), true).
```

Other plugins can then hook in:

```prolog
% In another plugin
:- multifile my_plugin:custom_hook/2.

my_plugin:custom_hook(Data, Context) :-
    % Custom behavior
    true.
```

### Resources

- [TerminusDB GitHub Repository](https://github.com/terminusdb/terminusdb)
- [TerminusDB Plugins Repository](https://github.com/terminusdb-labs/terminusdb-plugins)
- [Prolog Documentation](https://www.swi-prolog.org/pldoc/doc_for?object=manual)

### Community Plugins

Submit your plugins to the community repository at [terminusdb-labs/terminusdb-plugins](https://github.com/terminusdb-labs/terminusdb-plugins) to share with other users.

---

**Need help?** Join the [TerminusDB Discord community](https://discord.gg/terminusdb) or [open an issue on GitHub](https://github.com/terminusdb/terminusdb/issues).
