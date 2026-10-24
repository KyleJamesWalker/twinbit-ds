#!/bin/sh
# Runs on the device: pauses the stock frontend (and its idle auto-sleep) while the game runs.
# A deploy drops in twinbit.new and touches /tmp/twinbit.restart to relaunch without resuming the frontend.
# A crash also relaunches; a clean quit or the timeout ends the session.
# Set CAPTURE=1 to also log every input device and the lid sensor, and DEBUG=1 for remote control.
cd "$(dirname "$0")"
. /etc/profile.d/myenv.sh
front=$(pidof muos2.bin dmenu.bin)
[ -n "$front" ] && kill -STOP $front
trap '[ -n "$front" ] && kill -CONT $front; kill $(jobs -p) 2>/dev/null' EXIT INT TERM
if [ -n "$CAPTURE" ]; then
  for e in 0 2 4 6; do evtest /dev/input/event$e > ev$e.log 2>&1 & done
  (while :; do echo "$(date +%s) $(cat /sys/class/anbernic_misc/hallkey)"; sleep 0.5; done) > hall.log &
  export TWINBIT_LOG_INPUT=1
fi
[ -n "$DEBUG" ] && export TWINBIT_DEBUG=1
while :; do
  [ -f twinbit.new ] && mv twinbit.new twinbit
  rm -f /tmp/twinbit.restart
  timeout "${TIMEOUT:-1800}" ./twinbit > run.log 2>&1
  status=$?
  if [ ! -f /tmp/twinbit.restart ] && { [ $status -eq 0 ] || [ $status -eq 124 ]; }; then break; fi
  [ -f /tmp/twinbit.restart ] || { cp run.log crash-run.log; sleep 1; }
done
