#!/bin/bash
# AgentArk release-aware CLI for installed Docker deployments.

set -euo pipefail

SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="$(cd "${SOURCE_DIR}/.." && pwd)"
RELEASE_REPO="${AGENTARK_RELEASE_REPO:-agentark-ai/AgentArk}"
REPO_URL="https://github.com/${RELEASE_REPO}.git"
IMAGE_REPOSITORY="${AGENTARK_IMAGE_REPOSITORY:-ghcr.io/agentark-ai/agentark}"
UPDATE_CACHE_FILE="${INSTALL_DIR}/.agentark-update-check"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

docker_git_in_install() {
    docker run --rm -v "${INSTALL_DIR}:/work" -w /work alpine/git "$@"
}

latest_release_tag() {
    docker run --rm alpine/git ls-remote --tags --refs "${REPO_URL}" "v*" 2>/dev/null \
        | awk '{print $2}' \
        | sed 's#refs/tags/##' \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
        | awk -F'[v.]' '{printf("%09d.%09d.%09d %s\n", $2, $3, $4, $0)}' \
        | sort \
        | tail -n 1 \
        | awk '{print $2}'
}

release_version_from_tag() {
    printf '%s' "${1#v}"
}

current_release_tag() {
    docker_git_in_install git -C /work/source describe --tags --exact-match 2>/dev/null || true
}

ensure_clean_checkout() {
    local tracked_changes
    tracked_changes="$(docker_git_in_install git -C /work/source status --porcelain --untracked-files=no 2>/dev/null || true)"
    if [ -n "${tracked_changes}" ]; then
        echo -e "${YELLOW}Tracked local changes were found in ${SOURCE_DIR}. Resolve them before updating.${NC}" >&2
        exit 1
    fi
}

checkout_release_tag() {
    local release_tag="$1"
    ensure_clean_checkout
    docker_git_in_install git -C /work/source fetch --tags --force origin
    docker_git_in_install git -C /work/source checkout --force "${release_tag}"
}

run_start_script() {
    local release_tag
    release_tag="${AGENTARK_RELEASE_TAG:-$(current_release_tag)}"
    if [ -n "${release_tag}" ]; then
        (
            cd "${SOURCE_DIR}" &&
            AGENTARK_IMAGE="${IMAGE_REPOSITORY}:$(release_version_from_tag "${release_tag}")" \
            AGENTARK_RELEASE_REPO="${RELEASE_REPO}" \
            AGENTARK_RELEASE_TAG="${release_tag}" \
            ./scripts/start.sh "$@"
        )
    else
        (cd "${SOURCE_DIR}" && ./scripts/start.sh "$@")
    fi
}

cached_latest_release_tag() {
    local now cache_age cached_at cached_tag latest
    now="$(date +%s)"
    if [ -f "${UPDATE_CACHE_FILE}" ]; then
        IFS=' ' read -r cached_at cached_tag < "${UPDATE_CACHE_FILE}" || true
        if [ -n "${cached_at:-}" ] && [ -n "${cached_tag:-}" ]; then
            cache_age=$((now - cached_at))
            if [ "${cache_age}" -lt 86400 ]; then
                printf '%s\n' "${cached_tag}"
                return 0
            fi
        fi
    fi
    latest="$(latest_release_tag || true)"
    if [ -n "${latest}" ]; then
        printf '%s %s\n' "${now}" "${latest}" > "${UPDATE_CACHE_FILE}"
        printf '%s\n' "${latest}"
    fi
}

maybe_print_update_notice() {
    case "${1:-help}" in
        help|update|uninstall)
            return 0
            ;;
    esac
    local current_tag latest_tag
    current_tag="$(current_release_tag)"
    latest_tag="$(cached_latest_release_tag || true)"
    if [ -n "${current_tag}" ] && [ -n "${latest_tag}" ] && [ "${current_tag}" != "${latest_tag}" ]; then
        echo -e "${YELLOW}Update available:${NC} ${current_tag} -> ${latest_tag}. Run ${BOLD}agentark update${NC}."
    fi
}

show_help() {
    echo "AgentArk CLI"
    echo ""
    echo "Usage: agentark <command>"
    echo ""
    echo "  chat       Interactive CLI chat with your agent"
    echo "  pulse      Run ArkPulse health check"
    echo "  start      Start AgentArk (or 'tunnel' for remote access)"
    echo "  tunnel     Start with remote access"
    echo "  stop       Stop AgentArk"
    echo "  restart    Restart AgentArk"
    echo "  logs       View live logs"
    echo "  status     Show running containers"
    echo "  backup     Backup Docker volumes"
    echo "  update     Install the latest tagged release and restart"
    echo "  setup      Run setup wizard"
    echo "  uninstall  Stop and remove containers"
}

if [ ! -f "${SOURCE_DIR}/docker-compose.yml" ]; then
    echo -e "${YELLOW}AgentArk source checkout is missing at ${SOURCE_DIR}.${NC}" >&2
    exit 1
fi

maybe_print_update_notice "${1:-help}"

case "${1:-help}" in
    chat)
        docker exec -it agentark-control /app/agentark --chat
        ;;
    pulse)
        echo -e "${CYAN}Running ArkPulse health check...${NC}"
        docker exec agentark-control /app/agentark --pulse
        ;;
    start)
        run_start_script start
        ;;
    tunnel)
        run_start_script tunnel "${2:-}"
        ;;
    stop)
        run_start_script stop
        ;;
    restart)
        run_start_script restart
        ;;
    update)
        target_tag="${AGENTARK_RELEASE_TAG:-$(latest_release_tag)}"
        if [ -z "${target_tag}" ]; then
            echo -e "${YELLOW}Unable to resolve the latest tagged AgentArk release.${NC}" >&2
            exit 1
        fi
        echo -e "${CYAN}Updating AgentArk to ${target_tag}...${NC}"
        checkout_release_tag "${target_tag}"
        AGENTARK_RELEASE_TAG="${target_tag}" run_start_script update
        ;;
    logs)
        run_start_script logs
        ;;
    status)
        run_start_script status
        ;;
    backup)
        run_start_script backup
        ;;
    setup)
        docker exec -it agentark-control /app/agentark --setup
        ;;
    uninstall)
        echo -e "${YELLOW}This will stop AgentArk and remove containers.${NC}"
        echo -e "${BOLD}Your data volumes and source checkout will be preserved.${NC}"
        read -r -p "Continue? [y/N] " confirm
        if [ "${confirm}" = "y" ] || [ "${confirm}" = "Y" ]; then
            run_start_script stop
            rm -f /usr/local/bin/agentark 2>/dev/null || true
            echo -e "${GREEN}Removed. Data volumes kept. Source remains in ${SOURCE_DIR}.${NC}"
        fi
        ;;
    *)
        show_help
        ;;
esac
