#!/usr/bin/env bash
# discover-rails.sh — runtime Rails introspection that complements discover.sh.
# Uses `bin/rails runner` for questions only the loaded app can answer.
#
# Terminology:
#   includes  — broadest: any path (direct include/require, inheritance, hooks, runtime dispatch)
#   imports   — direct only: explicit `include X` or `require X` in source (delegates to discover.sh)
#   requires  — alias for imports
#   inherits  — indirect only: got the module via inheritance/hooks/dispatch, NOT a direct include

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(realpath "$SCRIPT_DIR/../..")"
DISCOVER_SH="$SCRIPT_DIR/discover.sh"

die() { echo "error: $*" >&2; exit 1; }

run_rails() {
    local code="$1"
    shift
    IMAROBOT="true" bin/rails runner "$code" "$@"
}

# ─────────────────────────────────────────────────────────
# Subcommand: includes
# All classes that have the module anywhere in their ancestor chain.
# ─────────────────────────────────────────────────────────
cmd_includes() {
    local module_name="${1:-}"
    [ -n "$module_name" ] || die "usage: discover-rails.sh includes <MODULE> [--json]"

    local format="text"
    [ "${2:-}" = "--json" ] && format="json"

    local code
    read -r -d '' code <<'RUBY' || true
module_name = ARGV[0]
format = ARGV[1] || "text"

begin
  mod = module_name.constantize
rescue NameError
  $stderr.puts "error: #{module_name} is not a known constant"
  exit 1
end

results = ApplicationRecord.descendants
  .reject(&:abstract_class?)
  .select { |k| k.ancestors.include?(mod) }
  .sort_by(&:name)

$stderr.puts "#{results.size} classes found"

if format == "json"
  require "json"
  puts JSON.pretty_generate(results.map { |k| { class: k.name, table: k.table_name } })
else
  results.each { |k| puts k.name }
end
RUBY

    run_rails "$code" "$module_name" "$format"
}

# ─────────────────────────────────────────────────────────
# Subcommand: imports (alias: requires)
# Classes that directly `include <MODULE>` in their source file.
# Delegates to discover.sh's static AST search.
# ─────────────────────────────────────────────────────────
cmd_imports() {
    local module_name="${1:-}"
    [ -n "$module_name" ] || die "usage: discover-rails.sh imports <MODULE>"

    bash "$DISCOVER_SH" imports "$module_name"
}

# ─────────────────────────────────────────────────────────
# Subcommand: inherits
# Classes that have the module in their ancestors but do NOT
# directly `include` it — they got it via inheritance, hooks,
# or runtime dispatch.
# ─────────────────────────────────────────────────────────
cmd_inherits() {
    local module_name="${1:-}"
    [ -n "$module_name" ] || die "usage: discover-rails.sh inherits <MODULE> [--json]"

    local format="text"
    [ "${2:-}" = "--json" ] && format="json"

    # Get direct importers from static analysis (file → class is inferred from filename)
    local direct_files
    direct_files="$(bash "$DISCOVER_SH" imports "$module_name" 2>/dev/null | jq -r '.[].file')"

    local code
    read -r -d '' code <<'RUBY' || true
module_name = ARGV[0]
format = ARGV[1] || "text"
direct_files = ARGV[2] || ""

begin
  mod = module_name.constantize
rescue NameError
  $stderr.puts "error: #{module_name} is not a known constant"
  exit 1
end

direct_set = direct_files.split("\n").map do |f|
  # app/models/foo_bar.rb → FooBar
  f.sub(%r{^app/models/}, "").sub(%r{\.rb$}, "").classify
end.to_set

results = ApplicationRecord.descendants
  .reject(&:abstract_class?)
  .select { |k| k.ancestors.include?(mod) && !direct_set.include?(k.name) }
  .sort_by(&:name)

$stderr.puts "#{results.size} classes found"

if format == "json"
  require "json"
  puts JSON.pretty_generate(results.map { |k| { class: k.name, table: k.table_name } })
else
  results.each { |k| puts k.name }
end
RUBY

    run_rails "$code" "$module_name" "$format" "$direct_files"
}

# ─────────────────────────────────────────────────────────
# Subcommand: namespaces
# Map the application's namespace structure: controllers,
# routes, views, and route files per namespace.
# ─────────────────────────────────────────────────────────
cmd_namespaces() {
    local target="" format="text"

    while [ $# -gt 0 ]; do
        case "$1" in
            --json) format="json" ;;
            *) target="$1" ;;
        esac
        shift
    done

    local code
    read -r -d '' code <<'RUBY' || true
require "json"

target = ARGV[0].to_s.empty? ? nil : ARGV[0]
format = ARGV[1] || "text"
repo_root = ARGV[2] || Dir.pwd

# Build namespace map from routes
namespaces = {}

Rails.application.routes.routes.each do |route|
  controller = route.defaults[:controller]
  next unless controller

  parts = controller.split("/")
  # Skip framework namespaces
  next if %w[action_mailbox active_storage rails devise].include?(parts.first)

  ns = parts.size > 1 ? parts.first : "_root"
  action = route.defaults[:action]
  verb = route.verb
  path = route.path.spec.to_s.sub("(.:format)", "")

  namespaces[ns] ||= { controllers: Set.new, routes: [] }
  namespaces[ns][:controllers] << controller
  namespaces[ns][:routes] << { verb: verb, path: path, to: "#{controller}##{action}" }
end

# Enrich with filesystem data
namespaces.each do |ns, data|
  data[:controllers] = data[:controllers].to_a.sort

  # Count views
  view_dir = File.join(repo_root, "app", "views", ns)
  data[:view_count] = if File.directory?(view_dir)
    Dir.glob(File.join(view_dir, "**", "*.{erb,jbuilder,turbo_stream.erb}")).size
  else
    0
  end

  # Route file
  route_file = File.join(repo_root, "config", "routes", "#{ns}.rb")
  data[:route_file] = File.exist?(route_file) ? "config/routes/#{ns}.rb" : nil

  data[:route_count] = data[:routes].size
end

if target
  ns_data = namespaces[target]
  unless ns_data
    $stderr.puts "error: namespace '#{target}' not found"
    $stderr.puts "available: #{namespaces.keys.sort.join(', ')}"
    exit 1
  end

  if format == "json"
    puts JSON.pretty_generate({ namespace: target }.merge(ns_data))
  else
    puts "namespace: #{target}"
    puts "route_file: #{ns_data[:route_file] || '(none)'}"
    puts "controllers: #{ns_data[:controllers].size}"
    ns_data[:controllers].each { |c| puts "  #{c}" }
    puts "routes: #{ns_data[:route_count]}"
    ns_data[:routes].each { |r| puts "  #{r[:verb].ljust(8)} #{r[:path].ljust(50)} #{r[:to]}" }
    puts "views: #{ns_data[:view_count]}"
  end
else
  # Summary of all namespaces
  if format == "json"
    summary = namespaces.sort.map do |ns, data|
      {
        namespace: ns,
        route_file: data[:route_file],
        controllers: data[:controllers].size,
        routes: data[:route_count],
        views: data[:view_count]
      }
    end
    puts JSON.pretty_generate(summary)
  else
    namespaces.sort.each do |ns, data|
      puts "#{ns.ljust(20)} #{data[:controllers].size.to_s.rjust(3)} controllers   #{data[:route_count].to_s.rjust(4)} routes   #{data[:view_count].to_s.rjust(4)} views   #{data[:route_file] || ''}"
    end
  end
end
RUBY

    run_rails "$code" "$target" "$format" "$REPO_ROOT"
}

# ─────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────
main() {
    local subcmd="${1:-}"
    shift 2>/dev/null || true

    case "$subcmd" in
        includes)          cmd_includes "$@" ;;
        imports|requires)  cmd_imports "$@" ;;
        inherits)          cmd_inherits "$@" ;;
        namespaces)        cmd_namespaces "$@" ;;
        *)
            echo "usage: discover-rails.sh <command> [args]" >&2
            echo "" >&2
            echo "commands:" >&2
            echo "  includes <MODULE> [--json]    All classes with module in ancestors" >&2
            echo "  imports <MODULE>              Direct include/require only (static)" >&2
            echo "  requires                      Alias for imports" >&2
            echo "  inherits <MODULE> [--json]    Indirect only (inheritance, hooks, dispatch)" >&2
            echo "  namespaces [NAME] [--json]    Map namespace structure (controllers, routes, views)" >&2
            exit 1
            ;;
    esac
}

main "$@"
