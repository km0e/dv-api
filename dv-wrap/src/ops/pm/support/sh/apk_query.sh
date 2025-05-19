apk update >/dev/null 2>&1
apk version $pkgs | awk -v "pkgs=$pkgs" '
BEGIN {
  split(pkgs, t, " ")
  for (n in t) {
    m[t[n]] = ""
  }
}
/^[^ ]+ policy:$/ {
  pkg = $1
  sub(" policy:", "", pkg)
  delete m[pkg]
}
END {
  u = ""
  for (i in m) {
    u = u " " i
  }
  printf u
}'
