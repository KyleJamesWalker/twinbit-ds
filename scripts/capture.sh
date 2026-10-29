#!/usr/bin/env bash
# Records or streams the device's screens and sound with its hardware H.264 encoder. RGDS_HOST overrides the target.
#   scripts/capture.sh record [both|top|bottom]   start recording to the SD card (default: both)
#   scripts/capture.sh stop                       stop recording and save it as an MP4
#   scripts/capture.sh stream [top|bottom|both]   play the screens live on this Mac (default: top); AUDIO=0 for less delay
#   scripts/capture.sh list                       list the recordings on the device
#   scripts/capture.sh pull [name] [dir]          copy a recording here (default: the newest), screens stacked
set -euo pipefail
host="${RGDS_HOST:-root@anbernic.local}"
dir="${CAPTURE_DIR:-/mnt/sdcard/captures}"
port=5000
# The encoder writes limited-range BT.601 but tags it as full-range RGB.
retag=h264_metadata=video_full_range_flag=0:matrix_coefficients=6:colour_primaries=1:transfer_characteristics=1
unrange=colorspace=all=bt601-6-525:iall=bt601-6-525:irange=tv:range=pc

device() {
  ssh -T "$host" "dir=$dir port=$port retag=$retag AUDIO=${AUDIO:-1} bash -s -- $*" <<'EOF'
set -euo pipefail
pidfile=/tmp/twinbit-capture.pid
connector() {
  local top other
  top=$(grep -o 'top=Some("[^"]*")' /userdata/twinbit/run.log 2>/dev/null | tail -1 | cut -d'"' -f2 || true)
  top=${top:-DSI-1}
  [ "$top" = DSI-1 ] && other=DSI-2 || other=DSI-1
  [ "$1" = top ] || top=$other
  modetest -M rockchip -c 2>/dev/null | awk -v n="$top" '$3 == "connected" && $4 == n { print $1 }'
}
sources() {
  local s c sink
  case "$1" in both) s="top bottom" ;; top|bottom) s=$1 ;; *) echo "unknown screen: $1" >&2; exit 1 ;; esac
  for s in $s; do
    c=$(connector "$s")
    [ -n "$c" ] || { echo "no $s screen" >&2; exit 1; }
    echo "kmssrc connector-id=$c ! mpph264enc gop=60 ! h264parse config-interval=-1 ! queue ! mux."
  done
  [ "$AUDIO" = 0 ] && return
  sink=$(pactl list sink-inputs | awk '/^\tSink:/ { s = $2 } /application.name = .*twinbit/ { print s; exit }')
  sink=$(pactl list short sinks | awk -v s="$sink" '$1 == s { print $2 }')
  echo "pulsesrc device=${sink:-$(pactl get-default-sink)}.monitor ! audio/x-raw,format=S16LE,rate=44100,channels=2 ! queue ! mux."
}
start() {
  local src
  [ -f $pidfile ] && kill -0 "$(cat $pidfile)" 2>/dev/null && { echo "already capturing; run stop first" >&2; exit 1; }
  src=$(sources "$screens")
  # shellcheck disable=SC2086
  setsid gst-launch-1.0 -q -e "$@" $src >/tmp/twinbit-capture.log 2>&1 </dev/null &
  echo $! > $pidfile
  sleep 1
  kill -0 $! 2>/dev/null || { grep -v '^mpp' /tmp/twinbit-capture.log >&2; exit 1; }
}
cmd=$1 screens=${2:-}
case "$cmd" in
  record)
    mkdir -p "$dir"
    free=$(df -m "$dir" | awk 'NR == 2 { print $4 }')
    [ "$free" -gt 1000 ] || { echo "only $free MB free in $dir" >&2; exit 1; }
    out="$dir/twinbit-$(date +%Y%m%d-%H%M%S).mkv"
    start matroskamux name=mux ! filesink location="$out"
    echo "$out" > /tmp/twinbit-capture.file
    echo "recording $screens to $out ($free MB free, about 80 MB a minute)"
    ;;
  stream)
    if [ "$AUDIO" = 0 ]; then start mpegtsmux name=mux ! tcpserversink host=0.0.0.0 port="$port" sync=false sync-method=next-keyframe
    else start matroskamux streamable=true name=mux ! tcpserversink host=0.0.0.0 port="$port" sync=false sync-method=next-keyframe; fi
    rm -f /tmp/twinbit-capture.file
    ;;
  stop)
    [ -f $pidfile ] || { echo "not capturing" >&2; exit 1; }
    pid=$(cat $pidfile)
    kill -INT "$pid" 2>/dev/null || true
    for _ in $(seq 100); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
    kill "$pid" 2>/dev/null || true
    rm -f $pidfile
    [ -f /tmp/twinbit-capture.file ] || exit 0
    mkv=$(cat /tmp/twinbit-capture.file)
    rm /tmp/twinbit-capture.file
    nice -n 19 ffmpeg -loglevel error -y -i "$mkv" -map 0 -c:v copy -bsf:v "$retag" -c:a aac -b:a 160k -movflags +faststart "${mkv%.mkv}.mp4"
    rm "$mkv"
    echo "${mkv%.mkv}.mp4"
    ;;
  list) ls -lh "$dir"/*.mp4 2>/dev/null || echo "no recordings in $dir" ;;
  newest) ls -t "$dir"/*.mp4 | head -1 ;;
esac
EOF
}

case "${1:-}" in
  record) device record "${2:-both}" ;;
  stop) device stop ;;
  list) device list ;;
  stream)
    screens="${2:-top}"
    device stream "$screens"
    trap 'device stop' EXIT
    url="tcp://$(ssh -G "$host" | awk '/^hostname / { print $2 }'):$port"
    play=(ffplay -loglevel error -fflags nobuffer -flags low_delay -framedrop -window_title "twinbit $screens")
    if [ "$screens" = both ]; then
      audio=(); [ "${AUDIO:-1}" = 0 ] || audio=(-map 0:a -c:a pcm_s16le)
      ffmpeg -loglevel fatal -fflags nobuffer -flags low_delay -i "$url" \
        -filter_complex "[0:v:0][0:v:1]vstack,${unrange}[v]" -map "[v]" "${audio[@]}" -c:v rawvideo -f nut - | "${play[@]}" -
    else
      "${play[@]}" -vf "$unrange" "$url"
    fi
    ;;
  pull)
    remote="${2:-$(device newest)}"
    [[ "$remote" == */* ]] || remote="$dir/$remote"
    out="${3:-.}/$(basename "$remote")"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    scp -q "$host:$remote" "$tmp/raw.mp4"
    if [ "$(ffprobe -v error -select_streams v -show_entries stream=index -of csv=p=0 "$tmp/raw.mp4" | wc -l)" -gt 1 ]; then
      ffmpeg -loglevel error -y -i "$tmp/raw.mp4" -filter_complex "[0:v:0][0:v:1]vstack,fps=60[v]" -map "[v]" -map '0:a?' \
        -c:v libx264 -preset veryfast -crf 18 -c:a copy -movflags +faststart "$out"
    else
      mv "$tmp/raw.mp4" "$out"
    fi
    echo "$out"
    ;;
  *) sed -n '2,7p' "$0" ;;
esac
