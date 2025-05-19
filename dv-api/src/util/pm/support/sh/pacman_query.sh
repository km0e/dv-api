$am -Sy --noconfirm >/dev/null 2>&1
$am -Q $pkgs | awk -v pkgs="$pkgs" '
BEGIN {
  split(pkgs, t, " ")
  for (n in t) {
    m[t[n]] = ""
  }
}
/^[^ ]+/{
  split($1, a, " ")
  delete m[a[1]]
}
END {
  u = ""
  for (i in m) {
    u = u " " i
  }
  printf u
}'
