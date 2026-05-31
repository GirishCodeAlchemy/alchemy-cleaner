#!/usr/bin/env bash
# =============================================================================
#  mac_health_check.sh — macOS System Health & Performance Diagnostic
#  Version : 2.0.0
#  Author  : Wibey AI Coding Assistant
#  Usage   : bash mac_health_check.sh [--sudo]
#            --sudo  : enables deeper thermal / power diagnostics (requires root)
# =============================================================================

# Intentionally NOT using -e: diagnostic tools must survive partial failures
# (missing battery, no dig, empty ps fields) rather than dying mid-report.
set -uo pipefail

# ─── ANSI Colours ─────────────────────────────────────────────────────────────
RESET="\033[0m"
BOLD="\033[1m"
DIM="\033[2m"
RED="\033[31m"
GREEN="\033[32m"
YELLOW="\033[33m"
BLUE="\033[34m"
CYAN="\033[36m"
WHITE="\033[37m"
BG_BLUE="\033[44m"
BG_RED="\033[41m"
BG_GREEN="\033[42m"
BG_YELLOW="\033[43m"

# ─── Icons ────────────────────────────────────────────────────────────────────
OK="✅"
WARN="⚠️ "
CRIT="❌"
INFO="ℹ️ "
ARROW="➜"
BULLET="•"

# ─── Report File ──────────────────────────────────────────────────────────────
REPORT_DIR="$HOME/Desktop"
TIMESTAMP=$(date "+%Y%m%d_%H%M%S")
REPORT_FILE="$REPORT_DIR/mac_health_report_$TIMESTAMP.txt"

# ─── Health Score (0–100, starts at 100, deductions applied) ──────────────────
HEALTH_SCORE=100
declare -a ISSUES=()
declare -a SOLUTIONS=()

USE_SUDO=false
if [[ "${1:-}" == "--sudo" ]]; then
  USE_SUDO=true
fi

# =============================================================================
#  HELPER FUNCTIONS
# =============================================================================

print_header() {
  echo ""
  echo -e "${BG_BLUE}${BOLD}${WHITE}  $1  ${RESET}"
  echo -e "${BLUE}$(printf '─%.0s' {1..72})${RESET}"
}

print_sub() {
  echo -e "\n${CYAN}${BOLD}  $1${RESET}"
  echo -e "${DIM}  $(printf '─%.0s' {1..60})${RESET}"
}

ok()   { echo -e "  ${OK}  ${GREEN}$1${RESET}"; }
warn() { echo -e "  ${WARN} ${YELLOW}$1${RESET}"; }
crit() { echo -e "  ${CRIT}  ${RED}$1${RESET}"; }
info() { echo -e "  ${INFO}  ${WHITE}$1${RESET}"; }
line() { echo -e "  ${DIM}${BULLET} $1${RESET}"; }

deduct() {
  local points=$1
  local reason=$2
  local fix=$3
  HEALTH_SCORE=$(( HEALTH_SCORE - points ))
  ISSUES+=("$reason")
  SOLUTIONS+=("$fix")
}

hr() { echo -e "${DIM}$(printf '─%.0s' {1..72})${RESET}"; }

tee_out() { tee -a "$REPORT_FILE"; }

# Redirect output to terminal (with colours) AND a clean plain-text report file
# The `sed` on the tee branch strips ANSI escape codes so the saved file is readable.
exec > >(tee >(sed $'s/\033\\[[0-9;]*[mK]//g' > "$REPORT_FILE")) 2>&1

# =============================================================================
#  BANNER
# =============================================================================

clear
echo ""
echo -e "${BOLD}${CYAN}"
cat << 'BANNER'
  ╔══════════════════════════════════════════════════════════════════════╗
  ║          🍎  macOS System Health & Performance Diagnostic           ║
  ║                         mac_health_check.sh                         ║
  ║                  Root-cause analysis • Read-only safe               ║
  ╚══════════════════════════════════════════════════════════════════════╝
BANNER
echo -e "${RESET}"
echo -e "  ${DIM}Report will be saved to: ${BOLD}$REPORT_FILE${RESET}"
echo -e "  ${DIM}Date/Time : $(date)${RESET}"
echo ""

# Important note about macOS memory behaviour
echo -e "${BG_YELLOW}${BOLD}  IMPORTANT NOTE ABOUT macOS MEMORY  ${RESET}"
echo -e "  ${YELLOW}macOS intentionally caches files in 'Inactive' memory for speed."
echo -e "  This is ${BOLD}NOT a problem${RESET}${YELLOW} — it frees itself instantly when any app needs RAM."
echo -e "  True memory pressure is measured via swap usage and memory compressor"
echo -e "  activity, NOT by inactive/cached bytes. This tool checks those correctly.${RESET}"
echo ""
hr

# =============================================================================
#  SECTION 1 — SYSTEM INFORMATION
# =============================================================================

print_header "1. SYSTEM INFORMATION"

# macOS Version
MACOS_VER=$(sw_vers -productVersion)
MACOS_NAME=$(awk '/SOFTWARE LICENSE AGREEMENT FOR macOS/' '/System/Library/CoreServices/Setup Assistant.app/Contents/Resources/en.lproj/OSXSoftwareLicense.rtf' 2>/dev/null | awk -F 'macOS ' '{print $NF}' | awk '{print $1}' || echo "Unknown")
BUILD=$(sw_vers -buildVersion)

info "macOS Version   : $MACOS_VER (Build $BUILD)"

# Chip / Architecture
CHIP=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Unknown")
ARCH=$(uname -m)
info "Processor       : $CHIP ($ARCH)"

# Detect Apple Silicon vs Intel
IS_APPLE_SILICON=false
if [[ "$ARCH" == "arm64" ]]; then
  IS_APPLE_SILICON=true
  info "Silicon Type    : Apple Silicon (arm64)"
else
  info "Silicon Type    : Intel x86_64"
fi

# RAM
TOTAL_RAM_BYTES=$(sysctl -n hw.memsize)
TOTAL_RAM_GB=$(echo "scale=1; $TOTAL_RAM_BYTES / 1073741824" | bc)
info "Total RAM       : ${TOTAL_RAM_GB} GB"

# CPU Cores
PCORES=$(sysctl -n hw.physicalcpu 2>/dev/null || echo "?")
LCORES=$(sysctl -n hw.logicalcpu 2>/dev/null || echo "?")
info "CPU Cores       : $PCORES physical / $LCORES logical"

# Uptime
UPTIME_RAW=$(uptime | sed 's/^.*up //' | sed 's/,.*load.*//')
info "System Uptime   : $UPTIME_RAW"

# Hostname
info "Hostname        : $(scutil --get ComputerName 2>/dev/null || hostname)"

# macOS version freshness check
MAJOR=$(echo "$MACOS_VER" | cut -d. -f1)
MINOR=$(echo "$MACOS_VER" | cut -d. -f2)
if [[ "$MAJOR" -lt 13 ]]; then
  warn "macOS $MACOS_VER is older than Ventura (13). Security patches may be unavailable."
  deduct 5 "Outdated macOS version ($MACOS_VER)" \
    "Update macOS: System Settings → General → Software Update. Running older macOS reduces performance optimisations and security patches."
else
  ok "macOS $MACOS_VER is a supported release."
fi

# =============================================================================
#  SECTION 2 — CPU ANALYSIS
# =============================================================================

print_header "2. CPU ANALYSIS"

print_sub "Load Averages"

LOAD=$(sysctl -n vm.loadavg | awk '{print $2, $3, $4}')
LOAD_1=$(echo $LOAD | awk '{print $1}')
LOAD_5=$(echo $LOAD | awk '{print $2}')
LOAD_15=$(echo $LOAD | awk '{print $3}')

info "Load Average    : 1m=$LOAD_1  5m=$LOAD_5  15m=$LOAD_15"
info "Logical Cores   : $LCORES (sustained load above $LCORES = saturated)"

# Compare 5m load to core count
LOAD_INT=$(echo "${LOAD_5:-0}" | awk -F. '{print $1}')
LOAD_INT=${LOAD_INT:-0}
LCORES=${LCORES:-1}
if [[ "$LOAD_INT" -gt "$LCORES" ]]; then
  crit "5-minute load ($LOAD_5) exceeds core count ($LCORES) — CPU is saturated."
  deduct 15 "CPU saturated: 5m load $LOAD_5 > $LCORES cores" \
    "Find the offending process: open Activity Monitor → CPU tab, sort by '% CPU' descending. If it's a background service you don't recognise, force-quit it and Google the process name."
elif [[ "$LOAD_INT" -ge "$LCORES" ]]; then
  warn "5-minute load ($LOAD_5) is near core count ($LCORES) — CPU is under stress."
  deduct 5 "CPU near saturation: 5m load $LOAD_5" \
    "Check Activity Monitor → CPU for processes consuming >50% CPU continuously."
else
  ok "CPU load is healthy (5m=$LOAD_5 vs $LCORES cores)."
fi

print_sub "Top CPU-Consuming Processes (sampled)"

echo ""
echo -e "  ${BOLD}${WHITE}  PID   %CPU  COMMAND${RESET}"
echo -e "  ${DIM}  $(printf '─%.0s' {1..48})${RESET}"
ps -Arco pid,pcpu,comm | head -11 | tail -10 | while read -r pid cpu cmd; do
  cpu_int=$(echo "$cpu" | awk -F. '{print $1}')
  if [[ "$cpu_int" -ge 80 ]]; then
    echo -e "  ${RED}  $pid   $cpu%   $cmd${RESET}"
  elif [[ "$cpu_int" -ge 40 ]]; then
    echo -e "  ${YELLOW}  $pid   $cpu%   $cmd${RESET}"
  else
    echo -e "  ${DIM}  $pid   $cpu%   $cmd${RESET}"
  fi
done

# =============================================================================
#  SECTION 3 — MEMORY ANALYSIS (THE REAL STORY)
# =============================================================================

print_header "3. MEMORY ANALYSIS"

echo -e "  ${DIM}(Inactive/cached memory is ${BOLD}healthy by design${RESET}${DIM} — macOS reclaims it instantly.)${RESET}"

print_sub "vm_stat — Detailed Memory Counters"

VM_STAT=$(vm_stat)

# Parse page size from vm_stat header — works on both Intel and Apple Silicon.
# Header format: "Mach Virtual Memory Statistics: (page size of 4096 bytes)"
# grep -oE '[0-9]+' on the first line safely extracts just the number.
PAGE_SIZE=$(vm_stat | head -1 | grep -oE '[0-9]+' | head -1)
PAGE_SIZE=${PAGE_SIZE:-4096}

pages_free=$(echo "$VM_STAT" | awk '/Pages free/{gsub(/\./,"",$NF); print $NF}')
pages_active=$(echo "$VM_STAT" | awk '/Pages active/{gsub(/\./,"",$NF); print $NF}')
pages_inactive=$(echo "$VM_STAT" | awk '/Pages inactive/{gsub(/\./,"",$NF); print $NF}')
pages_speculative=$(echo "$VM_STAT" | awk '/Pages speculative/{gsub(/\./,"",$NF); print $NF}')
pages_wired=$(echo "$VM_STAT" | awk '/Pages wired down/{gsub(/\./,"",$NF); print $NF}')
pages_compressed=$(echo "$VM_STAT" | awk '/Pages occupied by compressor/{gsub(/\./,"",$NF); print $NF}')
pages_compressor=$(echo "$VM_STAT" | awk '/Pages stored in compressor/{gsub(/\./,"",$NF); print $NF}')
swapins=$(echo "$VM_STAT"  | awk '/Swapins/{gsub(/\./,"",$NF); print $NF}')
swapouts=$(echo "$VM_STAT" | awk '/Swapouts/{gsub(/\./,"",$NF); print $NF}')

to_mb() {
  local pages=$1
  echo $(( pages * PAGE_SIZE / 1048576 ))
}

MB_FREE=$(to_mb "${pages_free:-0}")
MB_ACTIVE=$(to_mb "${pages_active:-0}")
MB_INACTIVE=$(to_mb "${pages_inactive:-0}")
MB_WIRED=$(to_mb "${pages_wired:-0}")
MB_COMPRESSED=$(to_mb "${pages_compressed:-0}")
MB_COMPRESSOR=$(to_mb "${pages_compressor:-0}")

info "Free (truly idle)  : ${MB_FREE} MB"
info "Active (in use)    : ${MB_ACTIVE} MB"
info "Inactive (cache)   : ${MB_INACTIVE} MB  ${DIM}← healthy; macOS file cache${RESET}"
info "Wired (kernel)     : ${MB_WIRED} MB"
info "In Compressor      : ${MB_COMPRESSED} MB  ${DIM}← compressed in RAM${RESET}"
info "Swap-ins since boot: ${swapins:-0}"
info "Swap-outs since boot: ${swapouts:-0}"

# Evaluate memory pressure ─────────────────────────────────────────────────
print_sub "Memory Pressure Assessment"

SWAP_OUT_INT="${swapouts:-0}"
SWAP_OUT_INT=${SWAP_OUT_INT:-0}
COMP_MB=${MB_COMPRESSED:-0}
TOTAL_GB_INT=$(echo "${TOTAL_RAM_GB:-0}" | awk -F. '{print $1}')
TOTAL_GB_INT=${TOTAL_GB_INT:-0}

# Rule 1: swap-outs
if [[ "$SWAP_OUT_INT" -gt 500000 ]]; then
  crit "Very high swap-out activity ($SWAP_OUT_INT) — system has been paging heavily to SSD."
  deduct 25 "Critical swap pressure: $SWAP_OUT_INT swap-outs" \
    "Close unused applications. If this happens regularly with ≤8 GB RAM, upgrading RAM is the most effective fix. On Apple Silicon, you cannot upgrade RAM post-purchase — consider a new Mac with 16+ GB."
elif [[ "$SWAP_OUT_INT" -gt 100000 ]]; then
  warn "Elevated swap-out activity ($SWAP_OUT_INT) — occasional paging detected."
  deduct 10 "Moderate swap pressure: $SWAP_OUT_INT swap-outs" \
    "Monitor which apps consume the most RAM in Activity Monitor → Memory tab. Browser tabs are common culprits — limit open tabs or use Safari instead of Chrome."
else
  ok "Swap-out activity is low ($SWAP_OUT_INT). No paging pressure."
fi

# Rule 2: compressor memory
if [[ "$COMP_MB" -gt 3000 ]]; then
  warn "Compressor is holding ${COMP_MB} MB — RAM is under sustained pressure."
  deduct 10 "High memory compression (${COMP_MB} MB)" \
    "macOS compresses memory to delay swapping. If you see this consistently, you're running near the RAM ceiling. Quit apps you don't use or restart the Mac to clear accumulated compression."
elif [[ "$COMP_MB" -gt 1000 ]]; then
  info "Moderate memory compression (${COMP_MB} MB) — normal for heavy workloads."
else
  ok "Memory compression is low (${COMP_MB} MB). RAM is comfortable."
fi

# Rule 3: system memory_pressure tool
print_sub "system_level Memory Pressure (memory_pressure tool)"
MEM_PRESSURE=$(memory_pressure 2>/dev/null | grep "System-wide memory free percentage" || echo "unavailable")
info "memory_pressure: $MEM_PRESSURE"

# Parse pressure level
if echo "$MEM_PRESSURE" | grep -q "Critical"; then
  crit "System reports CRITICAL memory pressure."
  deduct 20 "Critical memory pressure reported by OS" \
    "Immediately close memory-heavy apps. Restart if the system feels sluggish. Persistent critical pressure = you need more RAM."
elif echo "$MEM_PRESSURE" | grep -q "Warn"; then
  warn "System reports WARNING memory pressure."
  deduct 8 "Warning-level memory pressure" \
    "Close browser tabs, quit apps running in background (Slack, Teams, Spotify)."
else
  ok "Memory pressure is at a normal level."
fi

print_sub "Top Memory-Consuming Processes"

echo ""
echo -e "  ${BOLD}${WHITE}  PID   MEM(MB)  COMMAND${RESET}"
echo -e "  ${DIM}  $(printf '─%.0s' {1..52})${RESET}"
ps -Arco pid,rss,comm | sort -k2 -rn | head -11 | tail -10 | while read -r pid rss cmd; do
  mb=$(( rss / 1024 ))
  if [[ "$mb" -ge 2000 ]]; then
    echo -e "  ${RED}  $pid   ${mb} MB   $cmd${RESET}"
  elif [[ "$mb" -ge 800 ]]; then
    echo -e "  ${YELLOW}  $pid   ${mb} MB   $cmd${RESET}"
  else
    echo -e "  ${DIM}  $pid   ${mb} MB   $cmd${RESET}"
  fi
done

# =============================================================================
#  SECTION 4 — STORAGE & DISK HEALTH
# =============================================================================

print_header "4. STORAGE & DISK HEALTH"

print_sub "Boot Volume Space"

DISK_INFO=$(df -H / | tail -1)
DISK_TOTAL=$(echo "$DISK_INFO" | awk '{print $2}')
DISK_USED=$(echo "$DISK_INFO" | awk '{print $3}')
DISK_AVAIL=$(echo "$DISK_INFO" | awk '{print $4}')
DISK_PCT=$(echo "$DISK_INFO" | awk '{print $5}' | tr -d '%')
DISK_PCT=${DISK_PCT:-0}

info "Total Size      : $DISK_TOTAL"
info "Used            : $DISK_USED ($DISK_PCT% full)"
info "Available       : $DISK_AVAIL"

if [[ "$DISK_PCT" -ge 95 ]]; then
  crit "Disk is critically full ($DISK_PCT%). macOS needs free space to function. System will become unstable."
  deduct 30 "Disk critically full: $DISK_PCT% used" \
    "URGENT: Free up space immediately.
     → System Settings → General → Storage → Recommendations
     → Delete unused apps, large files, Downloads folder clutter
     → Empty Trash
     → Use 'brew cleanup' if Homebrew is installed
     → 'du -sh ~/Library/Caches/*' to find large caches (safe to delete)"
elif [[ "$DISK_PCT" -ge 85 ]]; then
  warn "Disk is $DISK_PCT% full. macOS requires ~10% free for efficient operation."
  deduct 15 "Disk nearly full: $DISK_PCT% used" \
    "Free up at least 20 GB:
     → System Settings → General → Storage for guided clean-up
     → Use DiskSight or DaisyDisk to visualise large files"
elif [[ "$DISK_PCT" -ge 70 ]]; then
  warn "Disk at $DISK_PCT%. Consider a clean-up before it becomes critical."
  deduct 5 "Disk at $DISK_PCT% capacity" \
    "Run System Settings → Storage → Recommendations for easy wins."
else
  ok "Disk usage is healthy ($DISK_PCT% used)."
fi

print_sub "S.M.A.R.T. Disk Health"

SMART_STATUS=$(diskutil info / 2>/dev/null | grep "SMART Status" | awk -F: '{print $2}' | xargs)
if [[ -z "$SMART_STATUS" ]]; then
  info "SMART status: Not available (may be APFS container or external drive)"
elif echo "$SMART_STATUS" | grep -qi "verified\|passed\|ok"; then
  ok "S.M.A.R.T. Status: $SMART_STATUS — drive health is Good."
elif echo "$SMART_STATUS" | grep -qi "fail"; then
  crit "S.M.A.R.T. Status: FAILING — drive has detected hardware errors."
  deduct 40 "S.M.A.R.T. disk FAILING" \
    "CRITICAL: Back up all data immediately to Time Machine or an external drive.
     The SSD/HDD may be approaching failure. Schedule a Genius Bar appointment or
     run Apple Diagnostics (hold D at startup) to confirm. This alone is a strong
     signal to erase & reinstall or replace hardware."
else
  warn "S.M.A.R.T. Status: $SMART_STATUS"
fi

print_sub "Disk I/O Pressure (iostat snapshot)"

# Quick 2-second iostat to detect sustained I/O wait
IO_SNAPSHOT=$(iostat -d 1 2 2>/dev/null | tail -1 || echo "unavailable")
if [[ "$IO_SNAPSHOT" != "unavailable" ]]; then
  IOKB=$(echo "$IO_SNAPSHOT" | awk '{print $3}')
  info "Recent disk I/O  : $IO_SNAPSHOT  (KB/s)"
else
  info "iostat snapshot  : unavailable on this macOS version"
fi

print_sub "Largest Directories in Home Folder"

echo -e "  ${DIM}(Top 10 space consumers under ~/)${RESET}"
du -sh ~/*/  2>/dev/null | sort -rh | head -10 | while read -r size path; do
  short="${path/#$HOME/~}"
  echo -e "  ${DIM}  $size   $short${RESET}"
done

# =============================================================================
#  SECTION 5 — STARTUP & BACKGROUND AGENTS
# =============================================================================

print_header "5. STARTUP ITEMS & BACKGROUND AGENTS"

print_sub "Login Items (GUI)"

LOGIN_ITEMS=$(osascript -e 'tell application "System Events" to get the name of every login item' 2>/dev/null || echo "")
if [[ -z "$LOGIN_ITEMS" ]]; then
  info "No GUI login items detected (or permission denied)."
else
  COUNT=$(echo "$LOGIN_ITEMS" | tr ',' '\n' | wc -l | tr -d ' ')
  if [[ "$COUNT" -gt 15 ]]; then
    crit "$COUNT login items — excessive startup apps will slow boot and consume RAM."
    deduct 10 "Excessive login items ($COUNT)" \
      "Remove unnecessary login items: System Settings → General → Login Items.
       Keep only items you actively need at startup."
  elif [[ "$COUNT" -gt 8 ]]; then
    warn "$COUNT login items found. Consider trimming."
    deduct 5 "Many login items ($COUNT)" \
      "Review System Settings → General → Login Items. Disable apps you don't need at every login."
  else
    ok "$COUNT login items (healthy)."
  fi
  echo -e "  ${DIM}  Items: $LOGIN_ITEMS${RESET}"
fi

print_sub "User LaunchAgents"

USER_AGENTS_DIR="$HOME/Library/LaunchAgents"
SYS_AGENTS_DIR="/Library/LaunchAgents"
SYS_DAEMONS_DIR="/Library/LaunchDaemons"

count_plist() {
  find "$1" -name "*.plist" -maxdepth 1 2>/dev/null | wc -l | tr -d ' '
}

UA_COUNT=$(count_plist "$USER_AGENTS_DIR")
SA_COUNT=$(count_plist "$SYS_AGENTS_DIR")
SD_COUNT=$(count_plist "$SYS_DAEMONS_DIR")

info "~/Library/LaunchAgents   : $UA_COUNT agents"
info "/Library/LaunchAgents    : $SA_COUNT agents"
info "/Library/LaunchDaemons   : $SD_COUNT agents"

UA_COUNT=${UA_COUNT:-0}; SA_COUNT=${SA_COUNT:-0}; SD_COUNT=${SD_COUNT:-0}
TOTAL_AGENTS=$(( UA_COUNT + SA_COUNT + SD_COUNT ))
if [[ "$TOTAL_AGENTS" -gt 50 ]]; then
  warn "Total LaunchAgents/Daemons: $TOTAL_AGENTS — unusually high; may include leftover software."
  deduct 5 "High number of background agents ($TOTAL_AGENTS)" \
    "Review ~/Library/LaunchAgents for orphaned plists from uninstalled apps.
     Use AppCleaner (free, macOS App Store) to fully remove apps including their agents."
else
  ok "Background agents count is normal ($TOTAL_AGENTS total)."
fi

print_sub "Running Services (launchctl list — user context)"

echo -e "  ${DIM}(Service count by status)${RESET}"
TOTAL_SERVICES=$(launchctl list 2>/dev/null | tail -n +2 | wc -l | tr -d ' ')
TOTAL_SERVICES=${TOTAL_SERVICES:-0}
FAILED_SERVICES=$(launchctl list 2>/dev/null | awk '$1 != "-" && $1 != "0" && $1 != "PID"' | tail -n +2 | wc -l | tr -d ' ')
FAILED_SERVICES=${FAILED_SERVICES:-0}

info "Total user services running : $TOTAL_SERVICES"
if [[ "$FAILED_SERVICES" -gt 10 ]]; then
  warn "$FAILED_SERVICES user services show non-zero exit codes (may indicate repeated crashes)."
  deduct 5 "Many failing user services ($FAILED_SERVICES)" \
    "Run 'launchctl list | awk \"\$1 != 0 && \$1 != -\"' to identify failing services.
     Non-zero exit services consume CPU on respawn loops."
else
  ok "$FAILED_SERVICES services with non-zero exits (normal range)."
fi

# =============================================================================
#  SECTION 6 — BATTERY HEALTH
# =============================================================================

print_header "6. BATTERY HEALTH"

BATT_INFO=$(system_profiler SPPowerDataType 2>/dev/null || echo "")

if [[ -z "$BATT_INFO" ]]; then
  info "Battery information not available (desktop Mac or profiler failed)."
else
  CYCLE_COUNT=$(echo "$BATT_INFO" | grep -i "Cycle Count" | awk '{print $NF}')
  CONDITION=$(echo "$BATT_INFO" | grep -i "Condition" | awk -F: '{print $2}' | xargs)
  MAX_CAPACITY=$(echo "$BATT_INFO" | grep -i "Maximum Capacity" | awk '{print $NF}')
  CHARGING=$(echo "$BATT_INFO" | grep -i "Connected" | head -1 | awk -F: '{print $2}' | xargs)

  info "Battery Condition   : ${CONDITION:-Unknown}"
  info "Cycle Count         : ${CYCLE_COUNT:-Unknown}"
  info "Maximum Capacity    : ${MAX_CAPACITY:-Unknown}"
  info "Power Adapter       : ${CHARGING:-Unknown}"

  if echo "$CONDITION" | grep -qi "replace"; then
    crit "Battery needs replacement — this degrades performance due to macOS power throttling."
    deduct 20 "Battery health: Replace Now/Soon" \
      "A degraded battery causes macOS to throttle the CPU to prevent sudden shutdowns.
       Book a Genius Bar appointment. Replacing the battery is far cheaper than a new Mac."
  elif [[ -n "$CYCLE_COUNT" ]] && [[ "$CYCLE_COUNT" -gt 1000 ]]; then
    warn "Cycle count $CYCLE_COUNT is above 1000. Battery is aging (designed for 1000 cycles)."
    deduct 5 "High battery cycle count ($CYCLE_COUNT)" \
      "Consider battery replacement if you notice capacity below 80% or unexpected shutdowns."
  elif [[ -n "$CYCLE_COUNT" ]] && [[ "$CYCLE_COUNT" -gt 700 ]]; then
    warn "Cycle count $CYCLE_COUNT — approaching end of rated lifespan."
  else
    ok "Battery health looks good (condition: ${CONDITION:-OK}, cycles: ${CYCLE_COUNT:-?})."
  fi
fi

# =============================================================================
#  SECTION 7 — SYSTEM LOGS: CRASHES & KERNEL PANICS
# =============================================================================

print_header "7. CRASH & KERNEL PANIC HISTORY"

print_sub "Kernel Panics (last 30 days)"

PANIC_DIR="/Library/Logs/DiagnosticReports"
PANIC_COUNT=$(find "$PANIC_DIR" -name "*.panic" -mtime -30 2>/dev/null | wc -l | tr -d ' ')
PANIC_COUNT=${PANIC_COUNT:-0}

if [[ "$PANIC_COUNT" -gt 5 ]]; then
  crit "$PANIC_COUNT kernel panics in the last 30 days — this is a serious instability signal."
  deduct 25 "Frequent kernel panics ($PANIC_COUNT in 30 days)" \
    "Kernel panics indicate driver conflicts, failing hardware, or corrupted system files.
     Steps:
     1. Run Apple Diagnostics (hold D at boot) to check hardware.
     2. Boot into Safe Mode (hold Shift at boot) — if stable, a third-party driver/kext is the culprit.
     3. Check Startup Disk: Disk Utility → First Aid on Macintosh HD.
     4. If persistent, reinstall macOS from Recovery (⌘+R)."
elif [[ "$PANIC_COUNT" -gt 1 ]]; then
  warn "$PANIC_COUNT kernel panics in the last 30 days. Investigate if system feels unstable."
  deduct 10 "Multiple kernel panics ($PANIC_COUNT in 30 days)" \
    "Boot into Safe Mode to isolate third-party drivers. Run Disk Utility First Aid on boot volume."
elif [[ "$PANIC_COUNT" -eq 1 ]]; then
  warn "1 kernel panic in the last 30 days. Isolated incidents are usually not alarming."
  deduct 2 "1 kernel panic (30 days)" \
    "Monitor if it recurs. Single panics can be caused by a force-shutdown or power surge."
else
  ok "No kernel panics in the last 30 days."
fi

print_sub "Application Crashes (last 7 days)"

CRASH_DIR="$HOME/Library/Logs/DiagnosticReports"
APP_CRASH_COUNT=$(find "$CRASH_DIR" -name "*.crash" -mtime -7 2>/dev/null | wc -l | tr -d ' ')
APP_CRASH_COUNT=${APP_CRASH_COUNT:-0}
TOP_CRASHES=$(find "$CRASH_DIR" -name "*.crash" -mtime -7 2>/dev/null | xargs -I{} basename {} .crash 2>/dev/null | sort | uniq -c | sort -rn | head -5)

info "App crash reports (7 days) : $APP_CRASH_COUNT"
if [[ "$APP_CRASH_COUNT" -gt 20 ]]; then
  warn "High application crash count ($APP_CRASH_COUNT in 7 days). System instability likely."
  deduct 10 "High app crash frequency ($APP_CRASH_COUNT in 7 days)" \
    "Check which app crashes most:
     ls -lt ~/Library/Logs/DiagnosticReports/*.crash | head -10
     Re-install the crashing app or check for macOS-incompatible versions."
fi

if [[ -n "$TOP_CRASHES" ]]; then
  echo -e "\n  ${DIM}Most frequent crashes:${RESET}"
  echo "$TOP_CRASHES" | while read -r line; do
    echo -e "  ${DIM}  $line${RESET}"
  done
fi

print_sub "Recent System Errors (log show — last 1 hour)"

RECENT_ERRORS=$(log show --last 1h --predicate 'eventType == logEvent AND messageType == error' --style compact 2>/dev/null | grep -v "^Filtering\|^---\|^Log" | wc -l | tr -d ' ')
RECENT_ERRORS=${RECENT_ERRORS:-0}
info "System error log entries (last 1h) : $RECENT_ERRORS"

if [[ "$RECENT_ERRORS" -gt 500 ]]; then
  warn "$RECENT_ERRORS error log entries in the last hour — elevated error rate."
  deduct 5 "High error log rate ($RECENT_ERRORS/hr)" \
    "Run: log show --last 1h --predicate 'messageType == error' --style compact | grep -v com.apple | head -30
     Focus on non-Apple subsystems, which point to third-party culprits."
else
  ok "Error log rate is within normal range ($RECENT_ERRORS entries/hr)."
fi

# =============================================================================
#  SECTION 8 — NETWORK STATUS
# =============================================================================

print_header "8. NETWORK STATUS"

print_sub "Active Network Interfaces"

ifconfig | grep -E "^en[0-9]|inet " | paste - - | while read -r iface inet ip rest; do
  echo -e "  ${DIM}  $(echo $iface | tr -d ':')  →  $ip${RESET}"
done

print_sub "DNS Responsiveness"

DNS_TIME=$(dig google.com +time=3 +tries=1 2>/dev/null | grep "Query time" | awk '{print $4, $5}')
if [[ -n "$DNS_TIME" ]]; then
  DNS_MS=$(echo "$DNS_TIME" | awk '{print $1}')
  DNS_MS=${DNS_MS:-0}
  info "DNS query time : $DNS_TIME"
  if [[ "$DNS_MS" -gt 500 ]]; then
    warn "DNS response time is high ($DNS_MS ms). This can make web browsing feel slow."
    deduct 5 "Slow DNS ($DNS_MS ms)" \
      "Switch to a faster DNS resolver:
       → System Settings → Network → [your connection] → DNS
       → Add 1.1.1.1 (Cloudflare) or 8.8.8.8 (Google) and remove existing entries."
  else
    ok "DNS response is fast ($DNS_MS ms)."
  fi
else
  info "DNS test skipped (dig not available or offline)."
fi

print_sub "Established Connections Count"

CONN_COUNT=$(netstat -an 2>/dev/null | grep ESTABLISHED | wc -l | tr -d ' ')
CONN_COUNT=${CONN_COUNT:-0}
info "Established TCP connections : $CONN_COUNT"
if [[ "$CONN_COUNT" -gt 200 ]]; then
  warn "$CONN_COUNT active TCP connections — unusual for typical use."
  deduct 5 "Unusually high TCP connections ($CONN_COUNT)" \
    "Run 'netstat -an | grep ESTABLISHED | awk '{print \$5}' | cut -d: -f1 | sort | uniq -c | sort -rn | head'
     to identify which remote hosts have the most connections."
else
  ok "TCP connection count looks normal ($CONN_COUNT)."
fi

# =============================================================================
#  SECTION 9 — OPTIONAL THERMAL / POWER (--sudo)
# =============================================================================

if $USE_SUDO; then
  print_header "9. THERMAL & POWER (Requires sudo)"

  if [[ "$EUID" -ne 0 ]]; then
    warn "Script was run without root privileges. Re-run with sudo to capture thermal data."
  else
    print_sub "powermetrics (2-second sample)"
    THERMAL=$(powermetrics -n 1 -i 2000 --samplers thermal 2>/dev/null | grep -E "CPU die temperature|GPU die temperature|Fan speed" || echo "unavailable")
    echo "$THERMAL" | while read -r line; do
      info "$line"
    done

    THROTTLE=$(powermetrics -n 1 -i 2000 --samplers cpu_power 2>/dev/null | grep -i "Thermal level\|throttle" || echo "none")
    if echo "$THROTTLE" | grep -qi "throttl"; then
      crit "CPU throttling detected: $THROTTLE"
      deduct 15 "Thermal throttling active" \
        "The Mac is reducing CPU speed to control heat.
         → Clean dust from vents (if MacBook Pro/Air, a can of compressed air on vents)
         → Use on a hard flat surface, not a bed or sofa
         → Check for runaway background processes (Activity Monitor)
         → SMC reset may help (Intel): Shift+Ctrl+Opt+Power on shutdown"
    else
      ok "No thermal throttling detected."
    fi
  fi
else
  print_header "9. THERMAL (--sudo mode not enabled)"
  info "Re-run with '--sudo' flag for thermal/power diagnostics:"
  info "  sudo bash mac_health_check.sh --sudo"
fi

# =============================================================================
#  FINAL SCORE & VERDICT
# =============================================================================

# Clamp score between 0 and 100
if [[ "$HEALTH_SCORE" -lt 0 ]]; then HEALTH_SCORE=0; fi
if [[ "$HEALTH_SCORE" -gt 100 ]]; then HEALTH_SCORE=100; fi

echo ""
echo ""
hr
echo -e "${BOLD}${BG_BLUE}${WHITE}  FINAL HEALTH SCORE & VERDICT  ${RESET}"
hr
echo ""

# Score bar
BAR_FILLED=$(( HEALTH_SCORE * 40 / 100 ))
BAR_EMPTY=$(( 40 - BAR_FILLED ))
BAR=""
for ((i=0; i<BAR_FILLED; i++)); do BAR="${BAR}█"; done
for ((i=0; i<BAR_EMPTY; i++)); do BAR="${BAR}░"; done

if [[ "$HEALTH_SCORE" -ge 80 ]]; then
  SCORE_COLOR=$GREEN
  VERDICT_ICON="✅"
  VERDICT="HEALTHY"
elif [[ "$HEALTH_SCORE" -ge 55 ]]; then
  SCORE_COLOR=$YELLOW
  VERDICT_ICON="⚠️ "
  VERDICT="NEEDS ATTENTION"
elif [[ "$HEALTH_SCORE" -ge 30 ]]; then
  SCORE_COLOR=$RED
  VERDICT_ICON="❌"
  VERDICT="DEGRADED — ACTION REQUIRED"
else
  SCORE_COLOR=$RED
  VERDICT_ICON="🚨"
  VERDICT="CRITICAL — CONSIDER REINSTALL"
fi

echo -e "  Health Score: ${SCORE_COLOR}${BOLD}${HEALTH_SCORE}/100${RESET}  ${SCORE_COLOR}[${BAR}]${RESET}"
echo -e "  Verdict     : ${SCORE_COLOR}${BOLD}$VERDICT_ICON  $VERDICT${RESET}"
echo ""

# ─── Format Recommendation ────────────────────────────────────────────────────

echo -e "${BOLD}  Should you erase and reinstall macOS?${RESET}"
echo -e "  ${DIM}(\"Erase All Content and Settings\" on Apple Silicon / Recovery Mode reinstall on Intel)${RESET}"
echo ""

# High-signal flags for reinstall recommendation
PANIC_SIGNAL=false
SMART_SIGNAL=false
SCORE_SIGNAL=false

if [[ "$PANIC_COUNT" -ge 5 ]]; then PANIC_SIGNAL=true; fi
if echo "${SMART_STATUS:-}" | grep -qi "fail"; then SMART_SIGNAL=true; fi
if [[ "$HEALTH_SCORE" -lt 30 ]]; then SCORE_SIGNAL=true; fi

if $SMART_SIGNAL; then
  echo -e "  ${BG_RED}${WHITE}${BOLD}  ⚠  HARDWARE ISSUE DETECTED  ${RESET}"
  echo -e "  ${RED}${BOLD}  S.M.A.R.T. failure indicates the physical drive may be failing."
  echo -e "  Erasing will NOT fix hardware. Back up data immediately and book a"
  echo -e "  Genius Bar appointment or contact Apple Support.${RESET}"
  echo ""
elif $PANIC_SIGNAL && $SCORE_SIGNAL; then
  echo -e "  ${BG_RED}${WHITE}${BOLD}  RECOMMENDATION: ERASE & REINSTALL macOS  ${RESET}"
  echo -e ""
  echo -e "  ${RED}Multiple critical signals (score=${HEALTH_SCORE}, $PANIC_COUNT kernel panics)."
  echo -e "  The system shows signs of deep instability unlikely to be fixed by"
  echo -e "  manual optimisation.${RESET}"
  echo ""
  echo -e "  ${BOLD}Steps to erase & reinstall:${RESET}"
  if $IS_APPLE_SILICON; then
    echo -e "  ${CYAN}  Apple Silicon (M1/M2/M3/M4):${RESET}"
    echo -e "  ${CYAN}  1. Back up to Time Machine or external drive first."
    echo -e "  ${CYAN}  2. System Settings → General → Transfer or Reset → Erase All Content and Settings."
    echo -e "  ${CYAN}  3. Follow the on-screen prompts. macOS reinstalls automatically.${RESET}"
  else
    echo -e "  ${CYAN}  Intel Mac:${RESET}"
    echo -e "  ${CYAN}  1. Back up to Time Machine or external drive first."
    echo -e "  ${CYAN}  2. Shut down. Power on and immediately hold ⌘+R to enter Recovery Mode."
    echo -e "  ${CYAN}  3. Select Disk Utility → Erase 'Macintosh HD' (APFS, GUID Partition Map)."
    echo -e "  ${CYAN}  4. Quit Disk Utility → Reinstall macOS.${RESET}"
  fi
elif $PANIC_SIGNAL; then
  echo -e "  ${YELLOW}Kernel panics are frequent enough to consider a reinstall, but your overall"
  echo -e "  score ($HEALTH_SCORE) suggests other fixes should be tried first:${RESET}"
  echo -e "  ${YELLOW}  1. Boot Safe Mode (hold Shift) to isolate kext/driver issues."
  echo -e "  2. Disk Utility → First Aid on boot volume."
  echo -e "  3. Run Apple Diagnostics (hold D at boot) to rule out hardware.${RESET}"
  echo -e "  If panics persist after those steps — proceed with erase & reinstall."
else
  echo -e "  ${GREEN}${BOLD}NOT recommended at this stage.${RESET}"
  echo -e "  ${GREEN}Score of $HEALTH_SCORE and no critical hardware/panic signals."
  echo -e "  Address the issues below — this should restore performance without an erase.${RESET}"
fi

# ─── Issues & Solutions Summary ───────────────────────────────────────────────

if [[ "${#ISSUES[@]}" -eq 0 ]]; then
  echo ""
  ok "No performance issues detected. System is in great shape! 🎉"
else
  echo ""
  echo ""
  echo -e "${BG_BLUE}${WHITE}${BOLD}  ISSUES FOUND & SOLUTIONS  ${RESET}"
  echo ""

  for i in "${!ISSUES[@]}"; do
    NUM=$(( i + 1 ))
    echo -e "  ${RED}${BOLD}Issue $NUM:${RESET} ${YELLOW}${ISSUES[$i]}${RESET}"
    echo -e "  ${GREEN}${BOLD}Fix   :${RESET}"
    # Wrap solution text with indent
    echo "${SOLUTIONS[$i]}" | while IFS= read -r sline; do
      echo -e "    ${sline}"
    done
    echo ""
  done
fi

# ─── Quick Win Commands ───────────────────────────────────────────────────────

echo ""
hr
echo -e "${BOLD}${CYAN}  ⚡  QUICK WIN COMMANDS (safe to run manually)${RESET}"
hr
echo ""
echo -e "  ${DIM}# Clear DNS cache${RESET}"
echo -e "  ${WHITE}sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder${RESET}"
echo ""
echo -e "  ${DIM}# Rebuild Spotlight index (if search feels slow)${RESET}"
echo -e "  ${WHITE}sudo mdutil -E /${RESET}"
echo ""
echo -e "  ${DIM}# Show top 10 memory consumers right now${RESET}"
echo -e "  ${WHITE}ps -Arco pid,rss,comm | sort -k2 -rn | head -11${RESET}"
echo ""
echo -e "  ${DIM}# Clear user font cache (fixes slow rendering)${RESET}"
echo -e "  ${WHITE}atsutil databases -remove${RESET}"
echo ""
echo -e "  ${DIM}# Check disk health in Disk Utility${RESET}"
echo -e "  ${WHITE}open /System/Applications/Utilities/Disk\\ Utility.app${RESET}"
echo ""
echo -e "  ${DIM}# Review startup login items${RESET}"
echo -e "  ${WHITE}open 'x-apple.systempreferences:com.apple.LoginItems-Settings.Extension'${RESET}"
echo ""
echo -e "  ${DIM}# Show large files (>500MB) in home folder${RESET}"
echo -e "  ${WHITE}find ~ -size +500M -not -path '*/.*' 2>/dev/null${RESET}"
echo ""

hr
echo -e "  ${DIM}Report saved to: ${BOLD}$REPORT_FILE${RESET}"
echo -e "  ${DIM}Script completed at: $(date)${RESET}"
hr
echo ""
