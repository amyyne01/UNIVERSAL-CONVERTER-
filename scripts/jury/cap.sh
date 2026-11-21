#!/bin/sh
# usage: cap.sh  (reads jobs "name|args" from stdin, runs 2 at a time)
cd "$1"
n=0
while IFS='|' read -r name args; do
  [ -z "$name" ] && continue
  ./AHG-Converter.exe $args --snap "$name.png" --tree "$name.json" >/dev/null 2>&1 &
  n=$((n+1)); if [ $n -ge 2 ]; then wait; n=0; fi
done
wait
