#!/usr/bin/env bash
# Memory and swap, every 2 s, into $R0_DIR/samples.tsv until killed.
# columns: epoch, used_mb, free_mb, compressed_mb, swap_used_mb, swapouts_total, top_rss_mb, top_comm, rustc_rss_sum_mb
out="$R0_DIR/samples.tsv"
printf 'epoch\tused_mb\tfree_mb\tcompressed_mb\tswap_used_mb\tswapouts_total\ttop_rss_mb\ttop_comm\trustc_rss_sum_mb\n' > "$out"
os=$(uname)
while :; do
  ts=$(python3 -c 'import time;print("%.0f"%time.time())')
  if [ "$os" = Darwin ]; then
    vm=$(vm_stat)
    page=$(echo "$vm" | sed -n 's/.*page size of \([0-9]*\) bytes.*/\1/p')
    g() { echo "$vm" | awk -F: -v k="$1" '$1==k {gsub(/[ .]/,"",$2); print $2}'; }
    free=$(g "Pages free"); act=$(g "Pages active"); wired=$(g "Pages wired down"); comp=$(g "Pages occupied by compressor"); so=$(g "Swapouts")
    used=$(( (act + wired + comp) * page / 1048576 )); freemb=$(( free * page / 1048576 )); compmb=$(( comp * page / 1048576 ))
    swap=$(sysctl -n vm.swapusage | sed -n 's/.*used = \([0-9.]*\)M.*/\1/p')
  else
    used=$(awk '/MemTotal/{t=$2}/MemAvailable/{a=$2}END{print int((t-a)/1024)}' /proc/meminfo)
    freemb=$(awk '/MemFree/{print int($2/1024)}' /proc/meminfo); compmb=0
    swap=$(awk '/SwapTotal/{t=$2}/SwapFree/{f=$2}END{print int((t-f)/1024)}' /proc/meminfo)
    so=$(awk '/pswpout/{print $2}' /proc/vmstat)
  fi
  top=$(ps -axo rss=,comm= 2>/dev/null | sort -rn | head -1); top=${top:-"0 none"}
  toprss=$(( $(echo $top | cut -d" " -f1) / 1024 )); topcomm=$(basename "$(echo "$top" | sed 's/^ *[0-9]* *//')")
  rsum=$(ps -axo rss=,comm= 2>/dev/null | awk '/rustc/{s+=$1}END{print int(s/1024)}')
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$ts" "$used" "$freemb" "$compmb" "${swap:-0}" "${so:-0}" "$toprss" "$topcomm" "$rsum" >> "$out"
  sleep 2
done
