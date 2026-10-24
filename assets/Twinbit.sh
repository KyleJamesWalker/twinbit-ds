#!/bin/bash
# Stock menu launcher: the frontend exits before this runs and restarts after.
cd "$(dirname "$0")/twinbit" || exit 1
. /etc/profile.d/myenv.sh
./twinbit > log.txt 2>&1
