MOD_DESC="Preflight: system packages, CPU virtualisation, KVM, RAM, disk (prints the sudo commands it needs)"

# command -> Debian/Ubuntu package. Required for adx itself.
_REQUIRED="bash:bash curl:curl wget:wget unzip:unzip git:git jq:jq perl:perl tar:tar xz:xz-utils sha256sum:coreutils awk:gawk pgrep:procps cc:build-essential"
# Used by status checks, the desktop menu entry and backups; adx degrades without them.
_OPTIONAL="lspci:pciutils notify-send:libnotify-bin desktop-file-validate:desktop-file-utils update-desktop-database:desktop-file-utils getfacl:acl rsync:rsync"

_missing_pkgs() {
  local pair cmd pkg out=""
  for pair in $1; do
    cmd="${pair%%:*}"; pkg="${pair#*:}"
    adx_have "$cmd" || case " $out " in *" $pkg "*) ;; *) out="$out $pkg" ;; esac
  done
  echo "${out# }"
}

_in_kvm_group_file() { getent group kvm | cut -d: -f4 | tr ',' '\n' | grep -qx "$(id -un)"; }
_in_kvm_group_session() { id -nG | tr ' ' '\n' | grep -qx kvm; }

mod_status() {
  local rc=0 req opt ram disk
  req="$(_missing_pkgs "$_REQUIRED")"; opt="$(_missing_pkgs "$_OPTIONAL")"
  if [ -z "$req" ]; then echo "required packages: ok"; else echo "required packages missing: $req"; rc=1; fi
  if [ -z "$opt" ]; then echo "optional packages: ok"; else echo "optional packages missing: $opt (desktop entry, notifications, GPU detection, backups)"; fi

  if grep -qE 'vmx|svm' /proc/cpuinfo; then echo "cpu virtualisation: ok"; else echo "cpu virtualisation: VT-x/AMD-V not visible (enable it in firmware; no nested VM)"; rc=1; fi
  if [ ! -e /dev/kvm ]; then
    echo "kvm: /dev/kvm missing (load kvm_intel / kvm_amd)"; rc=1
  elif [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
    if _in_kvm_group_session; then echo "kvm: ok (kvm group)"
    elif _in_kvm_group_file; then echo "kvm: ok via seat ACL; you are in the kvm group but this login predates it (log out and back in)"
    else echo "kvm: ok via seat ACL only (desktop session); for SSH/cron: sudo usermod -aG kvm \$USER, then log out and in"
    fi
  else
    echo "kvm: /dev/kvm not accessible: sudo usermod -aG kvm \$USER, then log out and in"; rc=1
  fi

  if [ "$(uname -m)" = x86_64 ]; then echo "arch: x86_64"; else echo "arch: $(uname -m) (adx downloads x86_64 builds only)"; rc=1; fi
  ram="$(free -m | awk '/^Mem:/{print $2}')"   # MiB: "8 GiB" machines report ~7.6 GiB
  if [ "$ram" -ge 15000 ]; then echo "ram: $((ram / 1024)) GiB"; elif [ "$ram" -ge 7500 ]; then echo "ram: $((ram / 1024)) GiB (16 GiB recommended)"; else echo "ram: $((ram / 1024)) GiB (too little for emulator + Gradle)"; rc=1; fi
  disk="$(df -BG --output=avail "$HOME" | tail -1 | tr -dc 0-9)"
  if [ "$disk" -ge 15 ]; then echo "disk free in \$HOME: ${disk} GiB"; else echo "disk free in \$HOME: ${disk} GiB (need ~15 GiB)"; rc=1; fi

  local w; w="$(cat /proc/sys/fs/inotify/max_user_watches 2>/dev/null || echo 0)"
  [ "$w" -ge 524288 ] && echo "inotify watches: $w" || echo "inotify watches: $w (only Android Studio needs 524288)"
  return $rc
}

# adx never runs sudo itself; print the exact commands instead.
mod_install() {
  local req opt
  req="$(_missing_pkgs "$_REQUIRED")"; opt="$(_missing_pkgs "$_OPTIONAL")"
  if [ -n "$req$opt" ]; then
    adx_warn "missing system packages; run:"
    echo "    sudo apt-get install -y $req${req:+ }$opt"
  fi
  if [ -e /dev/kvm ] && ! _in_kvm_group_file; then
    adx_warn "not in the kvm group; run: sudo usermod -aG kvm \$USER   (then log out and back in)"
  fi
  if ! adx_have claude; then adx_warn "Claude Code CLI not found; mobile-mcp needs it (https://claude.com/claude-code)"; fi
  [ -z "$req" ] || adx_die "install the required packages above, then rerun ./adx up"
  [ "$(uname -m)" = x86_64 ] || adx_die "x86_64 only (JDK, Node and system image downloads)"
  [ -e /dev/kvm ] || adx_die "/dev/kvm missing: enable VT-x/AMD-V in firmware, then: sudo modprobe kvm_intel (or kvm_amd)"
  grep -qE 'vmx|svm' /proc/cpuinfo || adx_die "CPU virtualisation not available; enable VT-x/AMD-V in firmware"
  adx_log "system preflight passed"
}
