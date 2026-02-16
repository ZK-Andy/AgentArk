#!/bin/bash
# AgentArk - Test Suite
# Run all tests: unit, integration, and API tests
#
# Auth: For API smoke tests, set AGENTARK_TEST_API_KEY or start the server
# with AGENTARK_INSECURE_NO_AUTH=true to bypass authentication.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║              AgentArk - Test Suite                        ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Track results
PASSED=0
FAILED=0

run_test() {
    local name="$1"
    local cmd="$2"

    echo -n "Testing $name... "
    if eval "$cmd" > /tmp/test_output.txt 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        cat /tmp/test_output.txt
        ((FAILED++))
    fi
}

# 1. Build check
echo "═══ Build Tests ═══"
run_test "cargo check" "cargo check"
run_test "cargo build (release)" "cargo build --release"
run_test "cargo clippy" "cargo clippy -- -D warnings"

# 2. Unit tests
echo ""
echo "═══ Unit Tests ═══"
run_test "cargo test" "cargo test --release"

# 3. Binary tests
echo ""
echo "═══ Binary Tests ═══"
BINARY="$PROJECT_ROOT/target/release/agentark"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    BINARY="$PROJECT_ROOT/target/release/agentark.exe"
fi

run_test "binary exists" "test -f '$BINARY'"
run_test "binary --help" "'$BINARY' --help"
run_test "binary --version" "'$BINARY' --version"

# 4. API tests (if server is running)
echo ""
echo "═══ API Tests ═══"

# Build auth header flag for curl
AUTH_HEADER=""
if [ -n "$AGENTARK_TEST_API_KEY" ]; then
    AUTH_HEADER="-H 'Authorization: Bearer $AGENTARK_TEST_API_KEY'"
fi

if curl -s http://127.0.0.1:8990/health > /dev/null 2>&1; then
    run_test "GET /health" "curl -sf http://127.0.0.1:8990/health"
    run_test "GET /status" "curl -sf $AUTH_HEADER http://127.0.0.1:8990/status"
    run_test "GET /skills" "curl -sf $AUTH_HEADER http://127.0.0.1:8990/skills"
    run_test "GET /tasks" "curl -sf $AUTH_HEADER http://127.0.0.1:8990/tasks"
    run_test "GET / (Web UI)" "curl -sf http://127.0.0.1:8990/ | grep -q 'AgentArk'"
else
    echo -e "${YELLOW}Skipping API tests (server not running)${NC}"
    echo "Start server with: $BINARY --headless"
fi

# 5. Docker tests
echo ""
echo "═══ Docker Tests ═══"
if command -v docker &> /dev/null; then
    run_test "Dockerfile syntax" "docker build --check . 2>/dev/null || docker build -t agentark-test . --target builder"
else
    echo -e "${YELLOW}Skipping Docker tests (docker not installed)${NC}"
fi

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════"
echo -e "Results: ${GREEN}$PASSED passed${NC}, ${RED}$FAILED failed${NC}"
echo "═══════════════════════════════════════════════════════════"

if [ $FAILED -gt 0 ]; then
    exit 1
fi
