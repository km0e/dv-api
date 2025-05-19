apt-get update -y >/dev/null 2>&1
apt-cache policy $pkgs | awk -v "pkgs=$pkgs" '
BEGIN {
  split(pkgs, t, " ")
  for (n in t) {
    m[t[n]] = ""
  }
}
/^[^ ]+:$/ {
  pkg = $1
  sub(":", "", pkg)
  delete m[pkg] 
}
END {
  u = ""
  for (i in m) {
    u = u " " i
  }
  printf u
}'
