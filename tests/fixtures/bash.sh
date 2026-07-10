#!/usr/bin/env bash

# Import/source statements
source ./lib/utils.sh
source "./lib/helpers.sh"
. ./lib/constants.sh
. "./lib/config.sh"

# Variable-expanded source paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/extra.sh"
source "${SCRIPT_DIR}/lib/more.sh"
. "$(dirname "$0")/lib/init.sh"

# Variable declarations
MY_CONST="hello"
export MY_EXPORT="world"
readonly MY_READONLY="immutable"
declare -r DECLARED_CONST="also_immutable"
declare -a MY_ARRAY=(one two three)
declare -A MY_ASSOC=([key1]=val1 [key2]=val2)

# Simple function (POSIX style)
my_posix_func() {
    echo "posix style"
}

# Function with keyword
function my_keyword_func {
    echo "keyword style"
}

# Function with keyword and parens
function my_both_func() {
    echo "both style"
}

# Function with local vars and logic
process_file() {
    local file="$1"
    local output="${file%.txt}.out"

    if [[ -f "$file" ]]; then
        while IFS= read -r line; do
            echo "$line"
        done < "$file"
    fi
}

# Conditional
if [[ "$1" == "run" ]]; then
    echo "running"
elif [[ "$1" == "test" ]]; then
    echo "testing"
fi

# Case statement
case "$1" in
    start)
        echo "starting"
        ;;
    stop)
        echo "stopping"
        ;;
    *)
        echo "unknown"
        ;;
esac

# Loops
for item in "$@"; do
    echo "$item"
done

while true; do
    break
done

# Pipeline and subshell
echo "hello" | grep -o "hell" | wc -c
result=$(command_here --flag)

# Trap and signal handling
trap 'cleanup' EXIT
trap 'echo caught' SIGINT

# Here document
cat <<'EOF'
This is a heredoc
EOF

# Array operations
arr=(one two three)
echo "${arr[@]}"
echo "${#arr[@]}"
